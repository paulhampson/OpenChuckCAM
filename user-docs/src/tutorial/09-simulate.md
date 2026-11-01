# 9. Simulate and read the results

The Simulate step plays the whole program back as a cut and compares the result with the model. The program first prepares the cut part, which takes a little longer than generating. [Simulation and deviation colours](../concepts/simulation.md) says more about the [simulation](../glossary.md#simulation).

1. Wait until the program has prepared the cut part of the whole program; the playback controls under the view are then active.
2. Use the playback controls under the view. The buttons go to the start, step back one move (<kbd>Left</kbd>), play or pause (<kbd>Space</kbd>), step on one move (<kbd>Right</kbd>), and go to the next operation (<kbd>Shift</kbd>+<kbd>Right</kbd>). **Speed** sets how fast it plays.
3. Go to the next operation to see the stock after roughing.

<figure>
  <img src="images/15-after-roughing.png" width="1366" height="768"
       alt="The stock part way through the program, at the start of the Finish operation: a rough cylinder shape with grooves, drawn translucent, with the held region at the left and the playback controls below.">
  <figcaption><b>Figure 1.</b> The stock after roughing, at the start of the Finish operation, with 0.3 mm left for the finishing tool.</figcaption>
</figure>

4. Set **Speed** to **10 000×** and press play, or drag the position bar to the end.

<figure>
  <img src="images/14-simulate-final.png" width="1366" height="768"
       alt="The cut part at the end of the program: the finished baluster shape, drawn translucent, with the held bar at the left. The results in the Inspector list the largest material left, the share within 0.1 mm and the material removed by each operation.">
  <figcaption><b>Figure 2.</b> The cut part at the end of the program, and the results in the Inspector.</figcaption>
</figure>

The results on the right say how closely the cut matches the model:

- **Largest overcut** (0.049 mm, within ±0.100 mm) is the most the tool cut below the model's surface. **Largest material left** (0.924 mm for this run) is the most material that stands above it, and **Within ±0.100 mm** (91.6 % of the sampled points) is the share that is on the surface to within 0.1 mm.
- The **deviation distribution** counts samples in each band of material left; below zero would mean cut *under* the surface.
- **By operation**: how many moves each makes and the volume it removes.
- **Cell size 0.100 mm** is the simulation's resolution. The stepped surface in the view is that cell size; it is not the finish the machine would leave. A finer cell is more accurate and slower.

Tick **Deviation colours** at the top of the Inspector to see where the material left is. Grey is on the surface, light blue has up to 1 mm left, and red would be cut below the surface.

<figure>
  <img src="../images/hero-2-deviation.png" width="1366" height="768"
       alt="The cut part in 3D, coloured by deviation. Most of the surface is grey. Light blue rings mark the shoulder, the collar, the cove and the end of the ball finial, where material is left. The legend at the lower left explains the colours.">
  <figcaption><b>Figure 3.</b> The cut part coloured by deviation. The blue rings are material left on the faces that look along the axis: the shoulder, the collar and the end of the finial.</figcaption>
</figure>

In this run helical finishing leaves material on those faces. Check the faces of your own parts the same way, and add an operation for any faces that matter. See the [known limitations](../start/limitations.md).

> [!WARNING]
> Treat the simulation as a check on the toolpaths, not a guarantee. It does not model the chuck, the holder or the machine, and it cannot show whether your tool, speed and feed will cut well.

The **Playback** panel also shows the programmed time: about 28 minutes 22 seconds here. This is the sum of the moves at their programmed feeds. It is not an estimate of the machine's time.

When you are satisfied, press **Post and export...** at the bottom right.
