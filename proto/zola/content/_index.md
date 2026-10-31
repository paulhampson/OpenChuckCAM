+++
title = "OpenChuckCAM user guide"
sort_by = "weight"

[extra]
headline = "Turn a 3D model into <span class=\"nowrap\">G-code</span> for 4-axis rotary machining"
lead = "OpenChuckCAM is free, open-source CAM software for machines that hold the work in a chuck and turn it while the cutter works around it, such as the Makera Z1. This guide is for anyone making parts with it, including people new to CAM."
hero_image = "images/hero-1-toolpaths.png"
hero_alt = "The OpenChuckCAM window on its Operations step. A turned baluster lies along the rotary axis, wrapped in the orange lines of its finishing toolpath, with dotted rapid moves and the held region at the chuck end."
hero_caption = "The Operations step: the finishing toolpath of the tutorial's baluster, in orange, around the part on the rotary axis. From a development build."
routes = [
  { title = "Start here", text = "Install the program and open it for the first time.", path = "@/start/install.md" },
  { title = "Your first project", text = "Take one part from a model file to G-code, in ten steps.", path = "@/tutorial/_index.md" },
]
+++

## Where to start

New to the program
: Read [Install and first run](@/start/install.md), then work through [Your first project](@/tutorial/_index.md). The tutorial takes one small part from a model file to G-code in ten steps, with a screenshot of each.

New to CAM or to 4-axis work
: The *Concepts* pages explain what a rotary axis does, what stock and a held region are, and why a job is usually cut in two passes, roughing and then finishing.

Ready to run a program on the Makera Z1
: Follow *Air cut on the Makera Z1* and the *Safe-practice checklist*. An air cut runs the program with no tool and no material, so you can watch the moves before anything can be damaged.

Looking for a setting or a word
: Use the search box, the [glossary](@/glossary.md), or the *Reference* pages.

## What the program does

| Step in the program | What you do there |
| --- | --- |
| Model | Import an STL or STEP file, check it, and place it on the rotary axis. |
| Setup | Choose the machine, describe the stock and how the work is held. |
| Operations | Choose tools and add the cutting operations, from roughing to finishing. |
| Simulate | Generate the toolpaths, watch the cut, and compare the result with the model. Then post and export the G-code and a setup sheet. |

## About this guide

This guide describes the development version of OpenChuckCAM. Each release has its own copy; use **Version** at the top of the page to switch. The guide is part of the project's source and is licensed AGPL-3.0-or-later. To report a mistake in it, open an issue in the project's repository.
