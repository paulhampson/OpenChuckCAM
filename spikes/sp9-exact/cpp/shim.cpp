// SPDX-License-Identifier: AGPL-3.0-only
// SP-9 spike, extends the SP-1 option B shim: a thin C ABI over OpenCASCADE 7.8.1.
// Uses only opencascade::handle<T> (never the Handle_T aliases, which are separate classes under MSVC)
// and plain C types at the boundary, so the Rust side needs no cxx and no opencascade-sys.
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPControl_Reader.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Pln.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>

#include <BRepClass_FaceClassifier.hxx>
#include <BRepAdaptor_Curve2d.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Surface.hxx>
#include <IntCurvesFace_Intersector.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TopExp_Explorer.hxx>
#include <gp_Lin.hxx>
#include <gp_Pnt2d.hxx>

#include <cstring>

namespace {
struct Model {
  TopoDS_Shape shape;
  TopTools_IndexedMapOfShape faces;  // index order = first-visit order of TopExp, duplicates removed
  TopTools_IndexedMapOfShape edges;
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
};

void put3(double* o, const gp_XYZ& v) { o[0] = v.X(); o[1] = v.Y(); o[2] = v.Z(); }

void set_err(char* buf, int len, const char* msg) {
  if (buf && len > 0) { std::strncpy(buf, msg, len - 1); buf[len - 1] = 0; }
}
}  // namespace

extern "C" {

// Silence OpenCASCADE's default printer (it writes reader diagnostics to stdout).
void occ_quiet() {
  const Handle(Message_Messenger)& m = Message::DefaultMessenger();
  m->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
}

// Read a STEP file. Returns a model or null; on null, err holds the reason.
// C++ exceptions are caught here. Hard faults (segfaults in the reader) are not: run this in a child process.
void* occ_read_step(const char* path, char* err, int err_len) {
  try {
    STEPControl_Reader reader;
    IFSelect_ReturnStatus st = reader.ReadFile(path);
    if (st != IFSelect_RetDone) { set_err(err, err_len, "STEP parse failed"); return nullptr; }
    reader.TransferRoots();
    TopoDS_Shape s = reader.OneShape();
    if (s.IsNull()) { set_err(err, err_len, "no shape transferred"); return nullptr; }
    Model* m = new Model();
    m->shape = s;
    TopExp::MapShapes(s, TopAbs_FACE, m->faces);
    TopExp::MapShapes(s, TopAbs_EDGE, m->edges);
    TopExp::MapShapesAndAncestors(s, TopAbs_EDGE, TopAbs_FACE, m->edge_faces);
    if (m->faces.Extent() == 0) { delete m; set_err(err, err_len, "file contains no faces"); return nullptr; }
    return m;
  } catch (const Standard_Failure& e) {
    set_err(err, err_len, e.GetMessageString());
  } catch (...) {
    set_err(err, err_len, "unknown C++ exception");
  }
  return nullptr;
}

void occ_free(void* m) { delete static_cast<Model*>(m); }
int occ_face_count(void* m) { return static_cast<Model*>(m)->faces.Extent(); }
int occ_edge_count(void* m) { return static_cast<Model*>(m)->edges.Extent(); }

// Face i (0-based): GeomAbs_SurfaceType, analytic parameters (out[12]), UV bounds (uv[4]), area, reversed.
int occ_face_info(void* mp, int i, double* out, double* uv, double* area, int* reversed) {
  Model* m = static_cast<Model*>(mp);
  const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
  BRepAdaptor_Surface s(f);
  BRepTools::UVBounds(f, uv[0], uv[1], uv[2], uv[3]);
  for (int k = 0; k < 12; ++k) out[k] = 0;
  switch (s.GetType()) {
    case GeomAbs_Plane: { gp_Pln p = s.Plane(); put3(out, p.Location().XYZ()); put3(out + 3, p.Axis().Direction().XYZ()); break; }
    case GeomAbs_Cylinder: { gp_Cylinder c = s.Cylinder(); put3(out, c.Location().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.Radius(); break; }
    case GeomAbs_Cone: { gp_Cone c = s.Cone(); put3(out, c.Apex().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.SemiAngle(); out[7] = c.RefRadius(); break; }
    case GeomAbs_Sphere: { gp_Sphere c = s.Sphere(); put3(out, c.Location().XYZ()); out[6] = c.Radius(); break; }
    case GeomAbs_Torus: { gp_Torus c = s.Torus(); put3(out, c.Location().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.MajorRadius(); out[7] = c.MinorRadius(); break; }
    default: break;
  }
  GProp_GProps props;
  BRepGProp::SurfaceProperties(f, props);
  *area = props.Mass();
  *reversed = f.Orientation() == TopAbs_REVERSED ? 1 : 0;
  return static_cast<int>(s.GetType());
}

// Point and unit outward normal at (u, v). Returns 0, or 1 if the normal is undefined there.
int occ_face_eval(void* mp, int i, double u, double v, double* pt, double* n) {
  Model* m = static_cast<Model*>(mp);
  const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
  BRepAdaptor_Surface s(f);
  gp_Pnt p; gp_Vec du, dv;
  s.D1(u, v, p, du, dv);
  gp_Vec nv = du.Crossed(dv);
  if (nv.Magnitude() < 1e-12) return 1;
  nv.Normalize();
  if (f.Orientation() == TopAbs_REVERSED) nv.Reverse();
  put3(pt, p.XYZ()); put3(n, nv.XYZ());
  return 0;
}

// Mesh with linear and angular deflection. Returns 0 on success.
int occ_mesh(void* mp, double lin, double ang, int parallel) {
  Model* m = static_cast<Model*>(mp);
  BRepTools::Clean(m->shape);
  BRepMesh_IncrementalMesh mesh(m->shape, lin, Standard_False, ang, parallel != 0);
  return mesh.IsDone() ? 0 : 1;
}

// Triangulation of face i: node and triangle counts. With non-null buffers, also copies nodes
// (xyz, model space) and triangles (0-based node indices, wound to match the face orientation).
int occ_face_mesh(void* mp, int i, int* nn, int* nt, double* nodes, int* tris) {
  Model* m = static_cast<Model*>(mp);
  const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
  TopLoc_Location loc;
  const Handle(Poly_Triangulation)& t = BRep_Tool::Triangulation(f, loc);
  if (t.IsNull()) { *nn = *nt = 0; return 1; }
  *nn = t->NbNodes(); *nt = t->NbTriangles();
  if (nodes) {
    const gp_Trsf tr = loc.Transformation();
    for (int k = 1; k <= *nn; ++k) { gp_Pnt p = t->Node(k).Transformed(tr); put3(nodes + 3 * (k - 1), p.XYZ()); }
  }
  if (tris) {
    bool rev = f.Orientation() == TopAbs_REVERSED;
    for (int k = 1; k <= *nt; ++k) {
      int a, b, c; t->Triangle(k).Get(a, b, c);
      tris[3 * (k - 1)] = a - 1;
      tris[3 * (k - 1) + 1] = (rev ? c : b) - 1;
      tris[3 * (k - 1) + 2] = (rev ? b : c) - 1;
    }
  }
  return 0;
}


// ---- SP-9 additions ----

// Full frame of an analytic face, so Rust can evaluate it with the same parametrisation as OpenCASCADE.
// out: origin(3), axis(3), xdir(3), a, b. Plane: a=b=0. Cylinder: a=R. Cone: a=reference radius, b=semi-angle.
// Sphere: a=R. Torus: a=major, b=minor. Returns the GeomAbs_SurfaceType.
int occ_face_frame(void* mp, int i, double* out) {
  Model* m = static_cast<Model*>(mp);
  const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
  BRepAdaptor_Surface s(f);
  for (int k = 0; k < 11; ++k) out[k] = 0;
  auto frame = [&](const gp_Ax3& ax) { put3(out, ax.Location().XYZ()); put3(out + 3, ax.Direction().XYZ()); put3(out + 6, ax.XDirection().XYZ()); };
  switch (s.GetType()) {
    case GeomAbs_Plane: frame(s.Plane().Position()); break;
    case GeomAbs_Cylinder: { gp_Cylinder c = s.Cylinder(); frame(c.Position()); out[9] = c.Radius(); break; }
    case GeomAbs_Cone: { gp_Cone c = s.Cone(); frame(c.Position()); out[9] = c.RefRadius(); out[10] = c.SemiAngle(); break; }
    case GeomAbs_Sphere: { gp_Sphere c = s.Sphere(); frame(c.Position()); out[9] = c.Radius(); break; }
    case GeomAbs_Torus: { gp_Torus c = s.Torus(); frame(c.Position()); out[9] = c.MajorRadius(); out[10] = c.MinorRadius(); break; }
    default: break;
  }
  return static_cast<int>(s.GetType());
}

// The face's surface as a B-spline (any non-analytic or Bezier surface converts), as plain data.
// meta: udeg, vdeg, nu poles, nv poles, rational, nuk (flat knots), nvk, uperiodic, vperiodic.
int occ_face_bspline(void* mp, int i, int* meta, double* uk, double* vk, double* poles, double* weights) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    Handle(Geom_Surface) surf = BRep_Tool::Surface(f);
    Handle(Geom_BSplineSurface) bs = Handle(Geom_BSplineSurface)::DownCast(surf);
    if (bs.IsNull()) bs = GeomConvert::SurfaceToBSplineSurface(surf);
    if (bs.IsNull()) return 1;
    // Periodic surfaces have no clamped knot vectors; unwrap a copy so the plain-data form is an ordinary patch.
    bs = Handle(Geom_BSplineSurface)::DownCast(bs->Copy());
    if (bs->IsUPeriodic()) bs->SetUNotPeriodic();
    if (bs->IsVPeriodic()) bs->SetVNotPeriodic();
    meta[0] = bs->UDegree(); meta[1] = bs->VDegree();
    meta[2] = bs->NbUPoles(); meta[3] = bs->NbVPoles();
    meta[4] = (bs->IsURational() || bs->IsVRational()) ? 1 : 0;
    meta[5] = bs->UKnotSequence().Length();
    meta[6] = bs->VKnotSequence().Length();
    meta[7] = bs->IsUPeriodic(); meta[8] = bs->IsVPeriodic();
    if (uk) {
      const TColStd_Array1OfReal& a = bs->UKnotSequence();
      for (int k = a.Lower(); k <= a.Upper(); ++k) uk[k - a.Lower()] = a(k);
      const TColStd_Array1OfReal& b = bs->VKnotSequence();
      for (int k = b.Lower(); k <= b.Upper(); ++k) vk[k - b.Lower()] = b(k);
      for (int a1 = 1; a1 <= meta[2]; ++a1)
        for (int b1 = 1; b1 <= meta[3]; ++b1) {
          int o = (a1 - 1) * meta[3] + (b1 - 1);
          gp_Pnt p = bs->Pole(a1, b1);
          poles[3 * o] = p.X(); poles[3 * o + 1] = p.Y(); poles[3 * o + 2] = p.Z();
          weights[o] = meta[4] ? bs->Weight(a1, b1) : 1.0;
        }
    }
    return 0;
  } catch (...) { return -1; }
}

// Closest point on the face's untrimmed surface. out: u, v, x, y, z, distance.
int occ_face_project(void* mp, int i, double x, double y, double z, double* out) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    Handle(Geom_Surface) surf = BRep_Tool::Surface(f);
    GeomAPI_ProjectPointOnSurf pr(gp_Pnt(x, y, z), surf);
    if (pr.NbPoints() < 1) return 1;
    double u, v; pr.LowerDistanceParameters(u, v);
    gp_Pnt q = pr.NearestPoint();
    out[0] = u; out[1] = v; out[2] = q.X(); out[3] = q.Y(); out[4] = q.Z(); out[5] = pr.LowerDistance();
    return 0;
  } catch (...) { return -1; }
}

// Distance from a point to the trimmed face (BRepExtrema). out: x, y, z, distance.
int occ_face_distance(void* mp, int i, double x, double y, double z, double* out) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    TopoDS_Vertex vtx = BRepBuilderAPI_MakeVertex(gp_Pnt(x, y, z));
    BRepExtrema_DistShapeShape d(vtx, f);
    if (!d.IsDone() || d.NbSolution() < 1) return 1;
    gp_Pnt q = d.PointOnShape2(1);
    out[0] = q.X(); out[1] = q.Y(); out[2] = q.Z(); out[3] = d.Value();
    return 0;
  } catch (...) { return -1; }
}

// Ray against the trimmed face. hits: up to 8 triples (t, u, v), sorted by t. Returns the number of hits.
int occ_face_ray(void* mp, int i, const double* o, const double* d, double tmax, double* hits) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    IntCurvesFace_Intersector inter(f, 1e-7);
    gp_Lin ln(gp_Pnt(o[0], o[1], o[2]), gp_Dir(d[0], d[1], d[2]));
    inter.Perform(ln, 0.0, tmax);
    int n = inter.NbPnt();
    int cnt = 0;
    for (int k = 1; k <= n && cnt < 8; ++k) {
      hits[3 * cnt] = inter.WParameter(k); hits[3 * cnt + 1] = inter.UParameter(k); hits[3 * cnt + 2] = inter.VParameter(k);
      ++cnt;
    }
    for (int a = 1; a < cnt; ++a)
      for (int b = a; b > 0 && hits[3 * b] < hits[3 * (b - 1)]; --b)
        for (int q = 0; q < 3; ++q) std::swap(hits[3 * b + q], hits[3 * (b - 1) + q]);
    return cnt;
  } catch (...) { return -1; }
}

// Is (u, v) inside the trimmed face (inside or on its boundary)? 1 yes, 0 no, -1 error.
int occ_face_contains(void* mp, int i, double u, double v) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    BRepClass_FaceClassifier fc(f, gp_Pnt2d(u, v), 1e-7);
    return (fc.State() == TopAbs_IN || fc.State() == TopAbs_ON) ? 1 : 0;
  } catch (...) { return -1; }
}

// The face's trim loops in (u, v): each edge's p-curve sampled at `per_edge` points. loop_starts receives the
// first point index of each wire and the total is returned. -1 on error, -2 if the capacity (points) is exceeded.
int occ_face_trims(void* mp, int i, int per_edge, double* uv, int cap, int* loop_starts, int* nloops) {
  try {
    Model* m = static_cast<Model*>(mp);
    const TopoDS_Face& f = TopoDS::Face(m->faces.FindKey(i + 1));
    int n = 0, w = 0;
    for (TopExp_Explorer we(f, TopAbs_WIRE); we.More(); we.Next(), ++w) {
      loop_starts[w] = n;
      for (BRepTools_WireExplorer ex(TopoDS::Wire(we.Current()), f); ex.More(); ex.Next()) {
        const TopoDS_Edge& e = ex.Current();
        BRepAdaptor_Curve2d c(e, f);
        double a = c.FirstParameter(), b = c.LastParameter();
        bool rev = e.Orientation() == TopAbs_REVERSED;
        for (int k = 0; k < per_edge; ++k) {
          double t = static_cast<double>(k) / per_edge;
          double p = rev ? b - t * (b - a) : a + t * (b - a);
          if (n >= cap) return -2;
          gp_Pnt2d q = c.Value(p);
          uv[2 * n] = q.X(); uv[2 * n + 1] = q.Y(); ++n;
        }
      }
    }
    *nloops = w;
    return n;
  } catch (...) { return -1; }
}

}  // extern "C"
