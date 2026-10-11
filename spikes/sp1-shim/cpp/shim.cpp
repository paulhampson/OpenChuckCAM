// SPDX-License-Identifier: AGPL-3.0-only
// SP-1 option B prototype: a thin C ABI over OpenCASCADE 7.8.1.
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

// Edge i (0-based): GeomAbs_CurveType, end points (ends[6]), up to 2 adjacent face ids (faces[2], -1 if none).
// Returns the number of adjacent faces.
int occ_edge_info(void* mp, int i, int* curve_type, double* ends, int* faces) {
  Model* m = static_cast<Model*>(mp);
  const TopoDS_Edge& e = TopoDS::Edge(m->edges.FindKey(i + 1));
  faces[0] = faces[1] = -1;
  if (BRep_Tool::Degenerated(e)) { *curve_type = -1; for (int k = 0; k < 6; ++k) ends[k] = 0; }
  else {
    BRepAdaptor_Curve c(e);
    *curve_type = static_cast<int>(c.GetType());
    put3(ends, c.Value(c.FirstParameter()).XYZ());
    put3(ends + 3, c.Value(c.LastParameter()).XYZ());
  }
  int n = 0;
  const TopTools_ListOfShape& fl = m->edge_faces.FindFromKey(e);
  for (TopTools_ListOfShape::Iterator it(fl); it.More(); it.Next()) {
    int id = m->faces.FindIndex(it.Value()) - 1;
    bool seen = false;
    for (int k = 0; k < n && k < 2; ++k) seen = seen || faces[k] == id;
    if (!seen) { if (n < 2) faces[n] = id; ++n; }
  }
  return n;
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

}  // extern "C"
