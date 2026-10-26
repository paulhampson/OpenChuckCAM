// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The STEP kernel wrapper (work breakdown slice 11 items 1 to 4; decision record 0003).
//
// A thin C ABI over OpenCASCADE 7.8.1. It reads a STEP file, numbers the faces, edges and vertices in
// TopExp order (so the same file always gives the same IDs, on every platform), converts every face to
// plain data (decision record 0017 item 2: an analytic frame and size, or a clamped, unwrapped rational
// B-spline patch, with the UV bounds, trim loops and a tessellation) and returns all of it as one JSON
// document shaped like openchuck_types::brep::BrepModel. Nothing else crosses the boundary: no handle, no
// kernel type, no pointer into the kernel.
//
// Rules from decision 0003: only opencascade::handle<T> (never the Handle_T aliases, which are separate
// classes under MSVC); exceptions are caught here and become error text; a file with no faces is an error.
// Hard faults inside the STEP reader are not catchable: the Rust side runs this in a child process.
#include <BRepAdaptor_Curve.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <IntCurvesFace_Intersector.hxx>
#include <gp_Lin.hxx>
#include <BRepAdaptor_Curve2d.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierCurve.hxx>
#include <GeomAbs_CurveType.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Surface.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Interface_Static.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPControl_Reader.hxx>
#include <Standard_Failure.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_SequenceOfAsciiString.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListIteratorOfListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
#include <gp_Trsf.hxx>

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

struct Result {
  std::string json;
  std::string error;
};

// A minimal JSON writer. Doubles are written with 17 significant digits so they read back exactly; the C
// locale is the default for a program that never calls setlocale, which this one does not.
class Json {
 public:
  std::string s;
  void raw(const char* t) { s += t; }
  void num(double v) {
    if (!std::isfinite(v)) throw std::runtime_error("a coordinate is not finite");
    if (v == 0.0) v = 0.0;  // no negative zero in the output
    char buf[40];
    std::snprintf(buf, sizeof buf, "%.17g", v);
    s += buf;
  }
  void integer(long long v) { s += std::to_string(v); }
  void boolean(bool v) { s += v ? "true" : "false"; }
  void str(const std::string& v) {
    s += '"';
    for (unsigned char c : v) {
      switch (c) {
        case '"': s += "\\\""; break;
        case '\\': s += "\\\\"; break;
        case '\n': s += "\\n"; break;
        default:
          if (c < 0x20) { char b[8]; std::snprintf(b, sizeof b, "\\u%04x", c); s += b; }
          else s += static_cast<char>(c);
      }
    }
    s += '"';
  }
  void point(double x, double y, double z) {
    s += "{\"x\":"; num(x); s += ",\"y\":"; num(y); s += ",\"z\":"; num(z); s += '}';
  }
};

void vec(Json& j, const gp_XYZ& v) { j.point(v.X(), v.Y(), v.Z()); }

void frame(Json& j, const gp_Ax3& a) {
  j.raw("\"frame\":{\"origin\":"); vec(j, a.Location().XYZ());
  j.raw(",\"x\":"); vec(j, a.XDirection().XYZ());
  j.raw(",\"y\":"); vec(j, a.YDirection().XYZ());
  j.raw(",\"z\":"); vec(j, a.Direction().XYZ());
  j.raw("}");
}

// The exact curve under an edge, as the importer's EdgeCurve (decision 0227), in the direction of the edge's
// polyline: `first` is the parameter at the polyline's first point. Writes `null` for a curve kind the importer
// does not read (the edge then keeps only its polyline). A B-spline is written only when it is clamped and not
// periodic, as the Rust side requires.
void edge_curve(Json& j, const TopoDS_Edge& e) {
  BRepAdaptor_Curve c(e);
  double a = c.FirstParameter(), b = c.LastParameter();
  if (e.Orientation() == TopAbs_REVERSED) std::swap(a, b);
  auto range = [&]() { j.raw(",\"first\":"); j.num(a); j.raw(",\"last\":"); j.num(b); };
  auto axes = [&](const gp_Ax2& ax) {
    j.raw("\"frame\":{\"origin\":"); vec(j, ax.Location().XYZ());
    j.raw(",\"x\":"); vec(j, ax.XDirection().XYZ());
    j.raw(",\"y\":"); vec(j, ax.YDirection().XYZ());
    j.raw(",\"z\":"); vec(j, ax.Direction().XYZ());
    j.raw("}");
  };
  switch (c.GetType()) {
    case GeomAbs_Line: {
      gp_Lin l = c.Line();
      j.raw("{\"kind\":\"line\",\"origin\":"); vec(j, l.Location().XYZ());
      j.raw(",\"direction\":"); vec(j, l.Direction().XYZ());
      range(); j.raw("}");
      return;
    }
    case GeomAbs_Circle: {
      gp_Circ ci = c.Circle();
      j.raw("{\"kind\":\"circle\","); axes(ci.Position());
      j.raw(",\"radius\":"); j.num(ci.Radius());
      range(); j.raw("}");
      return;
    }
    case GeomAbs_Ellipse: {
      gp_Elips el = c.Ellipse();
      j.raw("{\"kind\":\"ellipse\","); axes(el.Position());
      j.raw(",\"major\":"); j.num(el.MajorRadius());
      j.raw(",\"minor\":"); j.num(el.MinorRadius());
      range(); j.raw("}");
      return;
    }
    case GeomAbs_BSplineCurve:
    case GeomAbs_BezierCurve: {
      Handle(Geom_BSplineCurve) bs;
      if (c.GetType() == GeomAbs_BSplineCurve) {
        bs = c.BSpline();
      } else {
        Handle(Geom_BezierCurve) bz = c.Bezier();
        bs = GeomConvert::CurveToBSplineCurve(bz);
      }
      if (bs.IsNull() || bs->IsPeriodic()) { j.raw("null"); return; }
      int deg = bs->Degree(), np = bs->NbPoles();
      TColStd_Array1OfReal knots(1, bs->NbKnots());
      TColStd_Array1OfInteger mults(1, bs->NbKnots());
      bs->Knots(knots);
      bs->Multiplicities(mults);
      if (mults(1) != deg + 1 || mults(bs->NbKnots()) != deg + 1) { j.raw("null"); return; }
      j.raw("{\"kind\":\"b_spline\",\"degree\":"); j.integer(deg);
      j.raw(",\"knots\":[");
      bool firstk = true;
      for (int k = 1; k <= bs->NbKnots(); ++k)
        for (int m = 0; m < mults(k); ++m) { if (!firstk) j.raw(","); firstk = false; j.num(knots(k)); }
      j.raw("],\"poles\":[");
      for (int k = 1; k <= np; ++k) { if (k > 1) j.raw(","); gp_Pnt p = bs->Pole(k); j.point(p.X(), p.Y(), p.Z()); }
      j.raw("],\"weights\":[");
      for (int k = 1; k <= np; ++k) { if (k > 1) j.raw(","); j.num(bs->Weight(k)); }
      j.raw("]"); range(); j.raw("}");
      return;
    }
    default:
      j.raw("null");
  }
}

// Millimetres per unit for the length unit names OpenCASCADE reports (0 when it is not a unit we know).
double unit_millimetres(std::string name) {
  for (char& c : name) c = static_cast<char>(std::toupper(static_cast<unsigned char>(c)));
  static const std::map<std::string, double> table = {
      {"MM", 1.0},      {"MILLIMETRE", 1.0}, {"MILLIMETER", 1.0}, {"CM", 10.0},  {"CENTIMETRE", 10.0},
      {"CENTIMETER", 10.0}, {"M", 1000.0},   {"METRE", 1000.0},   {"METER", 1000.0}, {"KM", 1.0e6},
      {"UM", 0.001},    {"MICROMETRE", 0.001}, {"MICROMETER", 0.001}, {"INCH", 25.4}, {"IN", 25.4},
      {"FOOT", 304.8}, {"MIL", 0.0254},      {"MICROINCH", 0.0000254}, {"MILE", 1609344.0},
  };
  auto it = table.find(name);
  return it == table.end() ? 0.0 : it->second;
}

const char* kind_name(GeomAbs_SurfaceType t) {
  switch (t) {
    case GeomAbs_Plane: return "plane";
    case GeomAbs_Cylinder: return "cylinder";
    case GeomAbs_Cone: return "cone";
    case GeomAbs_Sphere: return "sphere";
    case GeomAbs_Torus: return "torus";
    case GeomAbs_BSplineSurface:
    case GeomAbs_BezierSurface: return "b_spline";
    default: return "other";
  }
}

struct Trims {
  std::vector<std::vector<gp_Pnt2d>> loops;
};

// Sample the face's trim loops in (u, v). Straight p-curves need only their start point; others get 48.
Trims sample_trims(const TopoDS_Face& f) {
  Trims out;
  for (TopExp_Explorer we(f, TopAbs_WIRE); we.More(); we.Next()) {
    std::vector<gp_Pnt2d> loop;
    for (BRepTools_WireExplorer ex(TopoDS::Wire(we.Current()), f); ex.More(); ex.Next()) {
      const TopoDS_Edge& e = ex.Current();
      BRepAdaptor_Curve2d c(e, f);
      double a = c.FirstParameter(), b = c.LastParameter();
      bool rev = e.Orientation() == TopAbs_REVERSED;
      int n = c.GetType() == GeomAbs_Line ? 1 : 48;
      for (int k = 0; k < n; ++k) {
        double t = static_cast<double>(k) / n;
        loop.push_back(c.Value(rev ? b - t * (b - a) : a + t * (b - a)));
      }
    }
    if (!loop.empty()) out.loops.push_back(std::move(loop));
  }
  return out;
}

// The face's surface as a clamped, unwrapped rational B-spline patch over the face's own parameter box
// (decision record 0017 item 2). Periodic directions are cut open at the face's parameter range, which can
// move the parameters by whole periods: *shift receives the amount added to u and v.
//
// A surface the kernel converts (revolution, extrusion, offset, ...) does not keep its parameters: a revolved
// surface's angle becomes the rational circle's parameter. The face's UV bounds and trim loops are in the
// original parameters, so for a converted surface `*remap` receives the patch and the caller maps every (u, v)
// to the patch by projecting the point the original surface gives there (`*converted` is then true).
void bspline_patch(Json& j, const TopoDS_Face& f, double uv[4], double shift[2], bool* converted,
                   Handle(Geom_BSplineSurface)* remap) {
  *converted = false;
  Handle(Geom_Surface) surf = BRep_Tool::Surface(f);
  Handle(Geom_BSplineSurface) bs = Handle(Geom_BSplineSurface)::DownCast(surf);
  if (bs.IsNull()) {
    // Revolution, extrusion and offset surfaces are infinite or periodic: convert only the part the face uses.
    Handle(Geom_Surface) part = new Geom_RectangularTrimmedSurface(surf, uv[0], uv[1], uv[2], uv[3]);
    bs = GeomConvert::SurfaceToBSplineSurface(part);
    *converted = true;
  }
  if (bs.IsNull()) throw std::runtime_error("the surface cannot be converted to a B-spline");
  bs = Handle(Geom_BSplineSurface)::DownCast(bs->Copy());
  shift[0] = shift[1] = 0.0;
  double u1, u2, v1, v2;
  bs->Bounds(u1, u2, v1, v2);
  const bool up = bs->IsUPeriodic(), vp = bs->IsVPeriodic();
  // A converted patch keeps no parameters from the original surface, so cutting it open at the face's
  // original range would be wrong; its trims are mapped by projection instead (remap_to_patch) and it is
  // only unwrapped below. Only a native B-spline is cut open at the face's range, which can shift parameters.
  if ((up || vp) && !*converted) {
    // Segment cuts a periodic direction open at [su1, su2]; the other direction keeps its full range.
    double su1 = up ? uv[0] : u1, su2 = up ? uv[1] : u2;
    double sv1 = vp ? uv[2] : v1, sv2 = vp ? uv[3] : v2;
    bs->Segment(su1, su2, sv1, sv2);
    double n1, n2, m1, m2;
    bs->Bounds(n1, n2, m1, m2);
    if (up) shift[0] = n1 - uv[0];
    if (vp) shift[1] = m1 - uv[2];
  }
  if (bs->IsUPeriodic()) bs->SetUNotPeriodic();
  if (bs->IsVPeriodic()) bs->SetVNotPeriodic();
  *remap = bs;
  const int nu = bs->NbUPoles(), nv = bs->NbVPoles();
  const TColStd_Array1OfReal& uk = bs->UKnotSequence();
  const TColStd_Array1OfReal& vk = bs->VKnotSequence();
  j.raw("\"geometry\":{\"kind\":\"b_spline\",\"degree_u\":"); j.integer(bs->UDegree());
  j.raw(",\"degree_v\":"); j.integer(bs->VDegree());
  j.raw(",\"knots_u\":[");
  for (int k = uk.Lower(); k <= uk.Upper(); ++k) { if (k > uk.Lower()) j.raw(","); j.num(uk(k)); }
  j.raw("],\"knots_v\":[");
  for (int k = vk.Lower(); k <= vk.Upper(); ++k) { if (k > vk.Lower()) j.raw(","); j.num(vk(k)); }
  j.raw("],\"poles_u\":"); j.integer(nu);
  j.raw(",\"poles_v\":"); j.integer(nv);
  j.raw(",\"poles\":[");
  const bool rational = bs->IsURational() || bs->IsVRational();
  for (int a = 1; a <= nu; ++a)
    for (int b = 1; b <= nv; ++b) {
      if (a > 1 || b > 1) j.raw(",");
      gp_Pnt p = bs->Pole(a, b);
      j.point(p.X(), p.Y(), p.Z());
    }
  j.raw("],\"weights\":[");
  for (int a = 1; a <= nu; ++a)
    for (int b = 1; b <= nv; ++b) {
      if (a > 1 || b > 1) j.raw(",");
      j.num(rational ? bs->Weight(a, b) : 1.0);
    }
  j.raw("]}");
}

// Maps the face's trim loops and UV bounds from the original surface's parameters to the converted patch's.
// Each point is projected onto the patch from the 3D point the original surface gives. A parameter on the box's
// edge maps to the patch's edge exactly, so a seam edge keeps its two ends (a projection cannot tell them apart).
void remap_to_patch(const BRepAdaptor_Surface& s, const Handle(Geom_BSplineSurface)& patch, const double orig[4],
                    Trims& trims, double uv[4]) {
  double pu0, pu1, pv0, pv1;
  patch->Bounds(pu0, pu1, pv0, pv1);
  const double eps_u = 1e-9 * (1.0 + std::abs(orig[1] - orig[0])), eps_v = 1e-9 * (1.0 + std::abs(orig[3] - orig[2]));
  auto map = [&](gp_Pnt2d& p) {
    const double u = p.X(), v = p.Y();
    gp_Pnt q = s.Value(u, v);
    GeomAPI_ProjectPointOnSurf pr(q, patch, pu0, pu1, pv0, pv1);
    if (pr.NbPoints() < 1) throw std::runtime_error("a trim point could not be mapped onto the converted surface");
    double nu, nv;
    pr.LowerDistanceParameters(nu, nv);
    if (std::abs(u - orig[0]) <= eps_u) nu = pu0;
    else if (std::abs(u - orig[1]) <= eps_u) nu = pu1;
    if (std::abs(v - orig[2]) <= eps_v) nv = pv0;
    else if (std::abs(v - orig[3]) <= eps_v) nv = pv1;
    p = gp_Pnt2d(nu, nv);
  };
  double lo_u = 1e300, hi_u = -1e300, lo_v = 1e300, hi_v = -1e300;
  for (auto& loop : trims.loops)
    for (auto& p : loop) {
      map(p);
      lo_u = std::min(lo_u, p.X()); hi_u = std::max(hi_u, p.X());
      lo_v = std::min(lo_v, p.Y()); hi_v = std::max(hi_v, p.Y());
    }
  if (lo_u <= hi_u) { uv[0] = lo_u; uv[1] = hi_u; uv[2] = lo_v; uv[3] = hi_v; }
  else { uv[0] = pu0; uv[1] = pu1; uv[2] = pv0; uv[3] = pv1; }
}

void write_model(Json& j, const TopoDS_Shape& shape, double lin, double ang) {
  TopTools_IndexedMapOfShape faces, edges, vertices;
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapes(shape, TopAbs_FACE, faces);
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  TopExp::MapShapes(shape, TopAbs_VERTEX, vertices);
  TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  if (faces.Extent() == 0) throw std::runtime_error("the file contains no faces");

  // Bodies: each solid in file order, then free shells, then free faces.
  std::vector<int> body(static_cast<size_t>(faces.Extent()), -1);
  int nbody = 0;
  auto assign = [&](const TopoDS_Shape& container) {
    bool any = false;
    for (TopExp_Explorer ex(container, TopAbs_FACE); ex.More(); ex.Next()) {
      int i = faces.FindIndex(ex.Current());
      if (i > 0 && body[static_cast<size_t>(i - 1)] < 0) { body[static_cast<size_t>(i - 1)] = nbody; any = true; }
    }
    if (any) ++nbody;
  };
  for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) assign(ex.Current());
  for (TopExp_Explorer ex(shape, TopAbs_SHELL, TopAbs_SOLID); ex.More(); ex.Next()) assign(ex.Current());
  for (TopExp_Explorer ex(shape, TopAbs_FACE, TopAbs_SHELL); ex.More(); ex.Next()) assign(ex.Current());
  for (size_t i = 0; i < body.size(); ++i) if (body[i] < 0) body[i] = nbody++;

  BRepTools::Clean(shape);
  BRepMesh_IncrementalMesh mesher(shape, lin, Standard_False, ang, Standard_False);
  if (!mesher.IsDone()) throw std::runtime_error("the tessellation failed");

  j.raw("{\"faces\":[");
  for (int fi = 1; fi <= faces.Extent(); ++fi) {
    const TopoDS_Face& f = TopoDS::Face(faces.FindKey(fi));
    try {
      BRepAdaptor_Surface s(f);
      double uv[4];
      BRepTools::UVBounds(f, uv[0], uv[1], uv[2], uv[3]);
      Trims trims = sample_trims(f);
      double orig_uv[4] = {uv[0], uv[1], uv[2], uv[3]};
      if (fi > 1) j.raw(",");
      j.raw("{\"id\":"); j.integer(fi - 1);
      j.raw(",\"surface\":\""); j.raw(kind_name(s.GetType()));
      j.raw("\",\"body\":"); j.integer(body[static_cast<size_t>(fi - 1)]);
      j.raw(",\"edges\":[");
      TopTools_IndexedMapOfShape own;
      for (TopExp_Explorer ex(f, TopAbs_EDGE); ex.More(); ex.Next()) {
        int e = edges.FindIndex(ex.Current());
        if (e > 0 && own.FindIndex(ex.Current()) == 0) {
          if (own.Extent() > 0) j.raw(",");
          own.Add(ex.Current());
          j.integer(e - 1);
        }
      }
      j.raw("],");
      double shift[2] = {0.0, 0.0};
      switch (s.GetType()) {
        case GeomAbs_Plane: j.raw("\"geometry\":{\"kind\":\"plane\","); frame(j, s.Plane().Position()); j.raw("}"); break;
        case GeomAbs_Cylinder: {
          gp_Cylinder c = s.Cylinder();
          j.raw("\"geometry\":{\"kind\":\"cylinder\","); frame(j, c.Position());
          j.raw(",\"radius\":"); j.num(c.Radius()); j.raw("}");
          break;
        }
        case GeomAbs_Cone: {
          gp_Cone c = s.Cone();
          j.raw("\"geometry\":{\"kind\":\"cone\","); frame(j, c.Position());
          j.raw(",\"radius\":"); j.num(c.RefRadius());
          j.raw(",\"semi_angle\":"); j.num(c.SemiAngle()); j.raw("}");
          break;
        }
        case GeomAbs_Sphere: {
          gp_Sphere c = s.Sphere();
          j.raw("\"geometry\":{\"kind\":\"sphere\","); frame(j, c.Position());
          j.raw(",\"radius\":"); j.num(c.Radius()); j.raw("}");
          break;
        }
        case GeomAbs_Torus: {
          gp_Torus c = s.Torus();
          j.raw("\"geometry\":{\"kind\":\"torus\","); frame(j, c.Position());
          j.raw(",\"major\":"); j.num(c.MajorRadius());
          j.raw(",\"minor\":"); j.num(c.MinorRadius()); j.raw("}");
          break;
        }
        default: {
          bool converted = false;
          Handle(Geom_BSplineSurface) patch;
          bspline_patch(j, f, uv, shift, &converted, &patch);
          if (converted) remap_to_patch(s, patch, orig_uv, trims, uv);
          break;
        }
      }
      j.raw(",\"reversed\":"); j.boolean(f.Orientation() == TopAbs_REVERSED);
      j.raw(",\"uv_bounds\":[");
      j.num(uv[0] + shift[0]); j.raw(","); j.num(uv[1] + shift[0]); j.raw(",");
      j.num(uv[2] + shift[1]); j.raw(","); j.num(uv[3] + shift[1]);
      j.raw("],\"trims\":[");
      for (size_t l = 0; l < trims.loops.size(); ++l) {
        if (l) j.raw(",");
        j.raw("[");
        for (size_t k = 0; k < trims.loops[l].size(); ++k) {
          if (k) j.raw(",");
          j.raw("{\"u\":"); j.num(trims.loops[l][k].X() + shift[0]);
          j.raw(",\"v\":"); j.num(trims.loops[l][k].Y() + shift[1]); j.raw("}");
        }
        j.raw("]");
      }
      j.raw("],\"tessellation\":{\"vertices\":[");
      TopLoc_Location loc;
      const Handle(Poly_Triangulation)& t = BRep_Tool::Triangulation(f, loc);
      if (t.IsNull()) throw std::runtime_error("the face has no tessellation");
      const gp_Trsf tr = loc.Transformation();
      for (int k = 1; k <= t->NbNodes(); ++k) {
        if (k > 1) j.raw(",");
        gp_Pnt p = t->Node(k).Transformed(tr);
        j.point(p.X(), p.Y(), p.Z());
      }
      j.raw("],\"triangles\":[");
      const bool rev = f.Orientation() == TopAbs_REVERSED;
      for (int k = 1; k <= t->NbTriangles(); ++k) {
        int a, b, c;
        t->Triangle(k).Get(a, b, c);
        if (k > 1) j.raw(",");
        j.raw("["); j.integer(a - 1); j.raw(","); j.integer((rev ? c : b) - 1);
        j.raw(","); j.integer((rev ? b : c) - 1); j.raw("]");
      }
      j.raw("]}}");
    } catch (const Standard_Failure& e) {
      throw std::runtime_error("face " + std::to_string(fi - 1) + ": " + e.GetMessageString());
    }
  }

  j.raw("],\"edges\":[");
  for (int ei = 1; ei <= edges.Extent(); ++ei) {
    const TopoDS_Edge& e = TopoDS::Edge(edges.FindKey(ei));
    if (ei > 1) j.raw(",");
    j.raw("{\"id\":"); j.integer(ei - 1);
    j.raw(",\"faces\":[");
    std::vector<int> fs;
    const TopTools_ListOfShape& lst = edge_faces.FindFromKey(e);
    for (TopTools_ListIteratorOfListOfShape it(lst); it.More(); it.Next()) fs.push_back(faces.FindIndex(it.Value()) - 1);
    std::sort(fs.begin(), fs.end());
    fs.erase(std::unique(fs.begin(), fs.end()), fs.end());
    for (size_t k = 0; k < fs.size(); ++k) { if (k) j.raw(","); j.integer(fs[k]); }
    j.raw("],\"vertices\":");
    TopoDS_Vertex v1, v2;
    TopExp::Vertices(e, v1, v2, Standard_True);
    if (v1.IsNull() || v2.IsNull()) j.raw("null");
    else { j.raw("["); j.integer(vertices.FindIndex(v1) - 1); j.raw(","); j.integer(vertices.FindIndex(v2) - 1); j.raw("]"); }
    j.raw(",\"polyline\":[");
    std::vector<gp_Pnt> pts;
    if (BRep_Tool::Degenerated(e)) {
      if (!v1.IsNull()) pts.push_back(BRep_Tool::Pnt(v1));
    } else {
      BRepAdaptor_Curve c(e);
      GCPnts_TangentialDeflection td(c, c.FirstParameter(), c.LastParameter(), ang, lin);
      for (int k = 1; k <= td.NbPoints(); ++k) pts.push_back(td.Value(k));
      if (e.Orientation() == TopAbs_REVERSED) std::reverse(pts.begin(), pts.end());
    }
    for (size_t k = 0; k < pts.size(); ++k) { if (k) j.raw(","); j.point(pts[k].X(), pts[k].Y(), pts[k].Z()); }
    j.raw("],\"curve\":");
    if (BRep_Tool::Degenerated(e)) j.raw("null");
    else edge_curve(j, e);
    j.raw("}");
  }

  j.raw("],\"vertices\":[");
  for (int vi = 1; vi <= vertices.Extent(); ++vi) {
    gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(vertices.FindKey(vi)));
    if (vi > 1) j.raw(",");
    j.raw("{\"id\":"); j.integer(vi - 1); j.raw(",\"point\":"); j.point(p.X(), p.Y(), p.Z()); j.raw("}");
  }
  j.raw("],\"tessellation\":{\"linear\":"); j.num(lin);
  j.raw(",\"angular\":"); j.num(ang); j.raw("}}");
}

Result* run(const char* path, double lin, double ang, double override_mm) {
  Result* r = new Result();
  try {
    if (!(lin > 0.0) || !(ang > 0.0)) throw std::runtime_error("the tessellation tolerances must be positive");
    Interface_Static::SetCVal("xstep.cascade.unit", "MM");
    STEPControl_Reader reader;
    IFSelect_ReturnStatus st = reader.ReadFile(path);
    if (st != IFSelect_RetDone) throw std::runtime_error("the STEP reader could not parse the file");
    TColStd_SequenceOfAsciiString lengths, angles, solid_angles;
    reader.FileUnits(lengths, angles, solid_angles);
    std::string unit_name = lengths.Length() > 0 ? std::string(lengths.Value(1).ToCString()) : std::string();
    double unit_mm = unit_name.empty() ? 1.0 : unit_millimetres(unit_name);
    reader.TransferRoots();
    TopoDS_Shape shape = reader.OneShape();
    if (shape.IsNull()) throw std::runtime_error("the file holds no shape");
    double scale = 1.0;
    if (override_mm > 0.0) {
      if (unit_mm <= 0.0) throw std::runtime_error("the file's own unit '" + unit_name + "' is not one we can convert from");
      scale = override_mm / unit_mm;
      gp_Trsf t;
      t.SetScale(gp_Pnt(0, 0, 0), scale);
      BRepBuilderAPI_Transform tf(shape, t, Standard_True);
      if (!tf.IsDone()) throw std::runtime_error("the model could not be scaled");
      shape = tf.Shape();
    }
    Json j;
    j.s.reserve(1 << 20);
    j.raw("{\"file_unit\":"); if (unit_name.empty()) j.raw("null"); else j.str(unit_name);
    j.raw(",\"file_unit_mm\":"); j.num(unit_mm);
    j.raw(",\"scale\":"); j.num(scale);
    j.raw(",\"model\":");
    write_model(j, shape, lin, ang);
    j.raw("}");
    r->json = std::move(j.s);
  } catch (const Standard_Failure& e) {
    r->error = e.GetMessageString() ? e.GetMessageString() : "the STEP kernel failed";
  } catch (const std::exception& e) {
    r->error = e.what();
  } catch (...) {
    r->error = "unknown C++ exception";
  }
  return r;
}

// The kernel as a test oracle: the same numbering as the import, but evaluation, projection, ray and trim tests
// done by OpenCASCADE itself, so tests can compare our pure-Rust evaluators with it (decision 0017, SP-9
// comparison kept as a regression test). Not used by the import.
struct Oracle {
  TopTools_IndexedMapOfShape faces;
};

}  // namespace

extern "C" {

// Silences OpenCASCADE's default printer, which writes reader diagnostics to standard output.
void occ_quiet() {
  const Handle(Message_Messenger)& m = Message::DefaultMessenger();
  m->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
}

// Reads the STEP file at `path` and tessellates it at `linear` mm and `angular` radians. `override_mm` is 0 to
// use the file's own unit, or the user's unit in millimetres to reinterpret the file's numbers in that unit.
// Always returns a result; see occ_result_error. C++ exceptions never leave this function.
void* occ_import_step(const char* path, double linear, double angular, double override_mm) {
  return run(path, linear, angular, override_mm);
}

// The error text, or null when the import worked.
const char* occ_result_error(const void* r) {
  const Result* res = static_cast<const Result*>(r);
  return res->error.empty() ? nullptr : res->error.c_str();
}

// The JSON document (valid until occ_result_free) and its length in bytes.
const char* occ_result_json(const void* r, size_t* len) {
  const Result* res = static_cast<const Result*>(r);
  *len = res->json.size();
  return res->json.c_str();
}

void occ_result_free(void* r) { delete static_cast<Result*>(r); }

// ---- Test oracle ----

void* occ_oracle_open(const char* path) {
  try {
    Interface_Static::SetCVal("xstep.cascade.unit", "MM");
    STEPControl_Reader reader;
    if (reader.ReadFile(path) != IFSelect_RetDone) return nullptr;
    reader.TransferRoots();
    TopoDS_Shape shape = reader.OneShape();
    if (shape.IsNull()) return nullptr;
    Oracle* o = new Oracle();
    TopExp::MapShapes(shape, TopAbs_FACE, o->faces);
    return o;
  } catch (...) { return nullptr; }
}

void occ_oracle_free(void* o) { delete static_cast<Oracle*>(o); }

static const TopoDS_Face* oracle_face(void* o, int i) {
  Oracle* m = static_cast<Oracle*>(o);
  if (i < 0 || i >= m->faces.Extent()) return nullptr;
  static thread_local TopoDS_Face f;
  f = TopoDS::Face(m->faces.FindKey(i + 1));
  return &f;
}

// Point and unit outward normal at (u, v): out = x y z nx ny nz. 0 on success, 1 where the normal is undefined.
int occ_oracle_eval(void* o, int i, double u, double v, double* out) {
  try {
    const TopoDS_Face* f = oracle_face(o, i);
    if (!f) return -1;
    BRepAdaptor_Surface s(*f);
    gp_Pnt p; gp_Vec du, dv;
    s.D1(u, v, p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (n.Magnitude() < 1e-12) return 1;
    n.Normalize();
    if (f->Orientation() == TopAbs_REVERSED) n.Reverse();
    out[0] = p.X(); out[1] = p.Y(); out[2] = p.Z(); out[3] = n.X(); out[4] = n.Y(); out[5] = n.Z();
    return 0;
  } catch (...) { return -1; }
}

// Closest point on the face's untrimmed surface: out = u v x y z distance.
int occ_oracle_project(void* o, int i, double x, double y, double z, double* out) {
  try {
    const TopoDS_Face* f = oracle_face(o, i);
    if (!f) return -1;
    Handle(Geom_Surface) surf = BRep_Tool::Surface(*f);
    GeomAPI_ProjectPointOnSurf pr(gp_Pnt(x, y, z), surf);
    if (pr.NbPoints() < 1) return 1;
    double u, v;
    pr.LowerDistanceParameters(u, v);
    gp_Pnt q = pr.NearestPoint();
    out[0] = u; out[1] = v; out[2] = q.X(); out[3] = q.Y(); out[4] = q.Z(); out[5] = pr.LowerDistance();
    return 0;
  } catch (...) { return -1; }
}

// Ray against the trimmed face: up to 8 (t, u, v) triples, nearest first. Returns the number of hits.
int occ_oracle_ray(void* o, int i, const double* org, const double* dir, double tmax, double* hits) {
  try {
    const TopoDS_Face* f = oracle_face(o, i);
    if (!f) return -1;
    IntCurvesFace_Intersector inter(*f, 1e-7);
    gp_Lin ln(gp_Pnt(org[0], org[1], org[2]), gp_Dir(dir[0], dir[1], dir[2]));
    inter.Perform(ln, 0.0, tmax);
    int cnt = 0;
    for (int k = 1; k <= inter.NbPnt() && cnt < 8; ++k, ++cnt) {
      hits[3 * cnt] = inter.WParameter(k); hits[3 * cnt + 1] = inter.UParameter(k); hits[3 * cnt + 2] = inter.VParameter(k);
    }
    for (int a = 1; a < cnt; ++a)
      for (int b = a; b > 0 && hits[3 * b] < hits[3 * (b - 1)]; --b)
        for (int q = 0; q < 3; ++q) std::swap(hits[3 * b + q], hits[3 * (b - 1) + q]);
    return cnt;
  } catch (...) { return -1; }
}

// Whether (u, v) lies on the trimmed face (inside or on its boundary): 1 yes, 0 no, -1 error.
int occ_oracle_contains(void* o, int i, double u, double v) {
  try {
    const TopoDS_Face* f = oracle_face(o, i);
    if (!f) return -1;
    BRepClass_FaceClassifier fc(*f, gp_Pnt2d(u, v), 1e-7);
    return (fc.State() == TopAbs_IN || fc.State() == TopAbs_ON) ? 1 : 0;
  } catch (...) { return -1; }
}

}  // extern "C"
