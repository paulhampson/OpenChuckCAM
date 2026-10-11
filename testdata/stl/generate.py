# SPDX-License-Identifier: AGPL-3.0-or-later
# Writes the STL corpus in this folder: python3 testdata/stl/generate.py testdata/stl
# See testdata/README.md for what each file holds and what it tests.
import struct, os, sys
out = sys.argv[1]
V = [(0,0,0),(1,0,0),(1,1,0),(0,1,0),(0,0,1),(1,0,1),(1,1,1),(0,1,1)]
T = [(0,2,1),(0,3,2),(4,5,6),(4,6,7),(0,1,5),(0,5,4),(2,3,7),(2,7,6),(1,2,6),(1,6,5),(0,4,7),(0,7,3)]
def normal(a,b,c):
    u=[b[i]-a[i] for i in range(3)]; v=[c[i]-a[i] for i in range(3)]
    n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
    l=sum(x*x for x in n)**0.5
    return [x/l for x in n] if l else [0,0,0]
def cube(side=10, offset=(0,0,0)):
    facets=[]
    for t in T:
        p=[tuple(V[i][k]*side+offset[k] for k in range(3)) for i in t]
        facets.append((normal(*p), p))
    return facets
def fmt(x):
    x=float(x)
    return str(int(x)) if x==int(x) else repr(x)
def ascii(facets, name="cube", nl="\n", first=None):
    s=[first if first is not None else f"solid {name}"]
    for n,p in facets:
        s.append("  facet normal "+" ".join(fmt(c) for c in n))
        s.append("    outer loop")
        for v in p: s.append("      vertex "+" ".join(fmt(c) for c in v))
        s.append("    endloop"); s.append("  endfacet")
    s.append(f"endsolid {name}")
    return (nl.join(s)+nl).encode()
def binary(facets, header=b"OpenChuckCAM test cube"):
    b=header.ljust(80,b"\0")[:80]+struct.pack("<I",len(facets))
    for n,p in facets:
        b+=struct.pack("<3f",*n)
        for v in p: b+=struct.pack("<3f",*v)
        b+=b"\0\0"
    return b
def w(name,data):
    open(os.path.join(out,name),"wb").write(data)
c=cube()
w("cube_ascii.stl", ascii(c))
w("cube_binary.stl", binary(c))
w("cube_binary_solid_header.stl", binary(c, b"solid cube exported with an ASCII-looking header"))
w("cube_nameless_crlf.stl", ascii(c, name="", nl="\r\n", first="solid"))
w("cube_inch.stl", ascii(cube(1), name="cube units=in"))
w("cube_metre_binary.stl", binary(cube(0.5), b"OpenChuckCAM test cube UNITS=m"))
w("cube_unknown_unit.stl", ascii(c, name="cube units=furlong"))
w("cube_small.stl", ascii(cube(1), name="small"))
w("cube_speck_binary.stl", binary(cube(0.05), b"OpenChuckCAM test cube, 0.05 units"))
w("cube_huge.stl", ascii(cube(5000), name="huge"))
w("two_cubes.stl", ascii(c, name="first")+ascii(cube(10,(20,0,0)), name="second"))
# defects
h=list(c); del h[2]; w("defect_hole.stl", ascii(h, name="hole"))
f=list(c); n,p=f[5]; f[5]=([-x for x in n],[p[0],p[2],p[1]]); w("defect_flipped.stl", ascii(f, name="flipped"))
m=list(c); n,p=m[3]; m[3]=([-x for x in n],p); w("defect_normal.stl", ascii(m, name="normal"))
io=[([-x for x in n],[p[0],p[2],p[1]]) for n,p in c]; w("defect_inside_out.stl", binary(io, b"inside out cube"))
nm=list(c)+[(normal((0,0,0),(10,0,0),(5,-10,-10)),[(0,0,0),(10,0,0),(5,-10,-10)])]; w("defect_non_manifold.stl", ascii(nm, name="fin"))
d=list(c)+[([0,0,0],[(0,0,0),(0,0,0),(0,0,0)])]; w("defect_degenerate.stl", ascii(d, name="degenerate"))
# damaged
b=binary(c); w("broken_truncated_binary.stl", b[:300])
a=ascii(c); w("broken_truncated_ascii.stl", a[:len(a)//2])
w("broken_bad_number.stl", a.replace(b"vertex 10 0 0", b"vertex 10 0 x", 1))
w("broken_bad_keyword.stl", a.replace(b"endloop", b"endlop", 1))
w("broken_garbage.stl", bytes(range(7, 47)))
w("broken_empty.stl", b"")
bb=bytearray(b); bb[84+12:84+16]=struct.pack("<f",float("nan")); w("broken_nan_binary.stl", bytes(bb))
w("broken_no_triangles.stl", binary([]))
