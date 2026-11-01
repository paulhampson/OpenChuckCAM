# 2. Import the model

1. Choose **Import model...**.
2. Type the path of `baluster.stl` (or use **Browse...**).
3. Press **Check and preview**.

<figure>
  <img src="images/03-import-model.png" width="1366" height="768"
       alt="The Import model dialog after checking: units in millimetres, 61 mm long, placement centred on the rotary axis, 5,376 triangles and no holes.">
  <figcaption><b>Figure 1.</b> The Import model dialog after the check: the units, the placement and the result of the mesh check.</figcaption>
</figure>

The dialog shows what the file holds before anything enters the project:

Units
: An STL file does not say what its numbers mean, so the program reads them as millimetres unless you choose another unit. The piece is 61 mm long and 22 mm across.

Placement
: "Centre on the rotary axis" puts the model's centre line on the A axis. The piece was made along X, so it needs nothing else.

Mesh check
: The program looks for holes, flipped faces and other faults. This model has none.

Then:

1. Press **Add to project**.
2. Press **Continue to setup** at the bottom right.
