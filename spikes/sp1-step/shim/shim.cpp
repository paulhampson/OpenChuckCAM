// SPDX-License-Identifier: AGPL-3.0-only
// SP-1 throwaway: minimal in-house C++ shim over the OCCT build that opencascade-sys already produces.
// Shows what a "thin in-house wrapper" fallback needs for B-rep surface data the opencascade crate does not bind.
#include <BRepAdaptor_Surface.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Pln.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
#include <gp_Vec.hxx>

static const TopoDS_Face& F(const void* p) { return TopoDS::Face(*static_cast<const TopoDS_Shape*>(p)); }
static void put3(double* o, const gp_XYZ& v) { o[0] = v.X(); o[1] = v.Y(); o[2] = v.Z(); }

extern "C" {

// Returns GeomAbs_SurfaceType (0 plane, 1 cylinder, 2 cone, 3 sphere, 4 torus, 5 bezier, 6 bspline, ...).
// out[0..8]: analytic parameters; uv[0..4]: umin, umax, vmin, vmax.
int shim_face_info(const void* face, double* out, double* uv) {
  const TopoDS_Face& f = F(face);
  BRepAdaptor_Surface s(f);
  BRepTools::UVBounds(f, uv[0], uv[1], uv[2], uv[3]);
  for (int i = 0; i < 12; ++i) out[i] = 0;
  switch (s.GetType()) {
    case GeomAbs_Plane: { gp_Pln p = s.Plane(); put3(out, p.Location().XYZ()); put3(out + 3, p.Axis().Direction().XYZ()); break; }
    case GeomAbs_Cylinder: { gp_Cylinder c = s.Cylinder(); put3(out, c.Location().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.Radius(); break; }
    case GeomAbs_Cone: { gp_Cone c = s.Cone(); put3(out, c.Apex().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.SemiAngle(); out[7] = c.RefRadius(); break; }
    case GeomAbs_Sphere: { gp_Sphere c = s.Sphere(); put3(out, c.Location().XYZ()); out[6] = c.Radius(); break; }
    case GeomAbs_Torus: { gp_Torus c = s.Torus(); put3(out, c.Location().XYZ()); put3(out + 3, c.Axis().Direction().XYZ()); out[6] = c.MajorRadius(); out[7] = c.MinorRadius(); break; }
    default: break;
  }
  return static_cast<int>(s.GetType());
}

// Point and unit normal at (u, v), normal flipped for reversed faces. Returns 0 on success, 1 if degenerate.
int shim_face_eval(const void* face, double u, double v, double* pt, double* n) {
  const TopoDS_Face& f = F(face);
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

// Mesh with linear AND angular deflection and optional parallel meshing; returns totals.
int shim_mesh(void* shape, double lin, double ang, int parallel, int* nodes, int* tris) {
  BRepMesh_IncrementalMesh m(*static_cast<TopoDS_Shape*>(shape), lin, Standard_False, ang, parallel != 0);
  int nn = 0, nt = 0;
  for (TopExp_Explorer ex(*static_cast<TopoDS_Shape*>(shape), TopAbs_FACE); ex.More(); ex.Next()) {
    TopLoc_Location loc;
    Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(TopoDS::Face(ex.Current()), loc);
    if (t.IsNull()) continue;
    nn += t->NbNodes(); nt += t->NbTriangles();
  }
  *nodes = nn; *tris = nt;
  return m.IsDone() ? 0 : 1;
}

}
