// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Writes the STEP fixtures that testdata/step/valid does not take from SP-1: a part in inches, a file of three
// separate solids, surfaces of revolution and extrusion (which the importer converts to B-splines), and a
// hexagonal bar whose flats are ruled walls for swarf, and a shouldered shaft with an inside corner for pencil
// clean-up.
// Every file is written by OpenCASCADE 7.8.1 itself, like the SP-1 fixtures (OI-25 is about files from other
// CAD tools).
//
// Build and run (OCCT_ROOT is the OpenCASCADE install, for example the occt-sys output directory):
//   g++ -std=c++17 -I$OCCT_ROOT/include make_fixtures.cpp -L$OCCT_ROOT/lib \
//     -Wl,--start-group -lTKDESTEP -lTKDE -lTKXCAF -lTKVCAF -lTKCAF -lTKLCAF -lTKCDF -lTKXSBase -lTKMesh \
//     -lTKShHealing -lTKBool -lTKBO -lTKFillet -lTKOffset -lTKPrim -lTKFeat -lTKTopAlgo -lTKGeomAlgo -lTKBRep \
//     -lTKGeomBase -lTKG3d -lTKG2d -lTKMath -lTKernel -Wl,--end-group -lpthread -ldl -o make_fixtures
//   ./make_fixtures testdata/step/valid
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <BRep_Builder.hxx>
#include <GeomAPI_PointsToBSpline.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Interface_Static.hxx>
#include <STEPControl_Writer.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include <cmath>
#include <iostream>
#include <string>

static bool write(const TopoDS_Shape& s, const std::string& path, const char* unit) {
  STEPControl_Writer w;  // creates the controller, which registers the write parameters
  Interface_Static::SetCVal("write.step.unit", unit);
  if (w.Transfer(s, STEPControl_AsIs) != IFSelect_RetDone) return false;
  return w.Write(path.c_str()) == IFSelect_RetDone;
}

static TopoDS_Shape moved(const TopoDS_Shape& s, double x, double y, double z) {
  gp_Trsf t;
  t.SetTranslation(gp_Vec(x, y, z));
  return BRepBuilderAPI_Transform(s, t, Standard_True).Shape();
}

int main(int argc, char** argv) {
  if (argc != 2) { std::cerr << "usage: make_fixtures <output dir>\n"; return 2; }
  const std::string dir = argv[1] + std::string("/");
  bool ok = true;

  // A cube whose numbers are 1.0 and whose unit is declared as inches: 25.4 mm once converted. The
  // writer only labels the unit, it does not scale, so the shape is built with side 1.
  ok &= write(BRepPrimAPI_MakeBox(1.0, 1.0, 1.0).Shape(), dir + "box_inch.step", "INCH");

  // Three separate solids in one file: a box, a cylinder and a sphere, 60 mm apart along X.
  {
    TopoDS_Compound c;
    BRep_Builder b;
    b.MakeCompound(c);
    b.Add(c, BRepPrimAPI_MakeBox(20, 20, 20).Shape());
    b.Add(c, moved(BRepPrimAPI_MakeCylinder(8, 30).Shape(), 60, 10, 0));
    b.Add(c, moved(BRepPrimAPI_MakeSphere(10).Shape(), 120, 10, 10));
    ok &= write(c, dir + "multi_body.step", "MM");
  }

  // A vase: a spline profile revolved about Z (a surface of revolution; periodic once converted).
  {
    TColgp_Array1OfPnt pts(1, 5);
    pts(1) = gp_Pnt(8, 0, 0);
    pts(2) = gp_Pnt(14, 0, 8);
    pts(3) = gp_Pnt(10, 0, 18);
    pts(4) = gp_Pnt(13, 0, 28);
    pts(5) = gp_Pnt(9, 0, 40);
    Handle(Geom_BSplineCurve) curve = GeomAPI_PointsToBSpline(pts).Curve();
    TopoDS_Edge e = BRepBuilderAPI_MakeEdge(curve).Edge();
    ok &= write(BRepPrimAPI_MakeRevol(e, gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), 2 * M_PI).Shape(),
                dir + "revolved_spline.step", "MM");
    // The same profile revolved through 200 degrees: a trimmed, non-periodic surface of revolution.
    ok &= write(BRepPrimAPI_MakeRevol(e, gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), 200.0 * M_PI / 180.0).Shape(),
                dir + "revolved_spline_partial.step", "MM");
  }

  // A spline wave extruded along Y (a surface of linear extrusion), as a closed solid.
  {
    TColgp_Array1OfPnt pts(1, 4);
    pts(1) = gp_Pnt(0, 0, 0);
    pts(2) = gp_Pnt(10, 0, 6);
    pts(3) = gp_Pnt(20, 0, -2);
    pts(4) = gp_Pnt(30, 0, 4);
    Handle(Geom_BSplineCurve) curve = GeomAPI_PointsToBSpline(pts).Curve();
    TopoDS_Edge top = BRepBuilderAPI_MakeEdge(curve).Edge();
    TopoDS_Edge right = BRepBuilderAPI_MakeEdge(gp_Pnt(30, 0, 4), gp_Pnt(30, 0, -10)).Edge();
    TopoDS_Edge bottom = BRepBuilderAPI_MakeEdge(gp_Pnt(30, 0, -10), gp_Pnt(0, 0, -10)).Edge();
    TopoDS_Edge left = BRepBuilderAPI_MakeEdge(gp_Pnt(0, 0, -10), gp_Pnt(0, 0, 0)).Edge();
    BRepBuilderAPI_MakeWire mw;
    mw.Add(top); mw.Add(right); mw.Add(bottom); mw.Add(left);
    TopoDS_Face face = BRepBuilderAPI_MakeFace(mw.Wire()).Face();
    ok &= write(BRepPrimAPI_MakePrism(face, gp_Vec(0, 25, 0)).Shape(), dir + "extruded_spline.step", "MM");
  }

  // A hexagonal bar along X (the rotary axis), 20 mm across the flats and 40 mm long, from X 0 to 40, with a
  // flat on top (Z 10). Each flat is a plane ruled across the axis, 11.547 mm wide: the swarf reference job
  // (decision record 0111).
  {
    const double r = 20.0 / std::sqrt(3.0);  // the circumradius
    BRepBuilderAPI_MakePolygon hexagon;
    for (int k = 0; k < 6; ++k) {
      const double t = k * M_PI / 3.0;
      hexagon.Add(gp_Pnt(0, r * std::cos(t), r * std::sin(t)));
    }
    hexagon.Close();
    TopoDS_Face end = BRepBuilderAPI_MakeFace(hexagon.Wire(), Standard_True).Face();
    ok &= write(BRepPrimAPI_MakePrism(end, gp_Vec(40, 0, 0)).Shape(), dir + "hex_bar.step", "MM");
  }

  // A shouldered shaft along Z from 0 to 40, the closed form of openchuck_testkit::brep::shouldered_shaft
  // (decision record 0226, OI-234 (b)): radius 8 to Z 12, a sharp shoulder out to radius 12 (the one inside
  // corner, 90 degrees), radius 12 to Z 24, a shoulder in to radius 10, a quarter-round fillet of radius 2
  // down to radius 8 at Z 26 (tangent at both ends), and radius 8 to Z 40. The profile in the XZ plane is
  // revolved about Z.
  {
    const gp_Pnt p[] = {gp_Pnt(0, 0, 0),   gp_Pnt(8, 0, 0),   gp_Pnt(8, 0, 12),  gp_Pnt(12, 0, 12),
                        gp_Pnt(12, 0, 24), gp_Pnt(10, 0, 24), gp_Pnt(8, 0, 26),  gp_Pnt(8, 0, 40),
                        gp_Pnt(0, 0, 40)};
    const double h = 2.0 * (1.0 - std::sqrt(0.5));
    BRepBuilderAPI_MakeWire mw;
    for (int k = 0; k < 8; ++k) {
      if (k == 5) {
        // The fillet: centre (10, 26), from (10, 24) to (8, 26) through the point at 45 degrees.
        Handle(Geom_TrimmedCurve) arc = GC_MakeArcOfCircle(p[5], gp_Pnt(8 + h, 0, 24 + h), p[6]).Value();
        mw.Add(BRepBuilderAPI_MakeEdge(arc).Edge());
      } else {
        mw.Add(BRepBuilderAPI_MakeEdge(p[k], p[k + 1]).Edge());
      }
    }
    mw.Add(BRepBuilderAPI_MakeEdge(p[8], p[0]).Edge());
    TopoDS_Face profile = BRepBuilderAPI_MakeFace(mw.Wire(), Standard_True).Face();
    ok &= write(BRepPrimAPI_MakeRevol(profile, gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), 2 * M_PI).Shape(),
                dir + "shouldered_shaft.step", "MM");
  }

  std::cerr << (ok ? "wrote the fixtures\n" : "FAILED\n");
  return ok ? 0 : 1;
}
