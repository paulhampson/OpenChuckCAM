+++
title = "OpenChuckCAM user guide"
sort_by = "weight"
+++

OpenChuckCAM turns a 3D model into G-code for 4-axis rotary machining: the work is held at one end in a chuck and turned about its length while the cutter moves along and around it. The first machine it supports is the Makera Z1.

This guide is for people who want to make parts with the program, including people who have not used [CAM](@/glossary.md#cam) software before. It explains each term the first time it is used and collects them in the [glossary](@/glossary.md).

> [!WARNING]
> Generated G-code must be checked by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. The program is in early development, and very little of its output has been run on a real machine. Read *Checking G-code before you run it* before you cut anything.

## Where to start

New to the program
: Read *Install and first run*, then work through [Your first project](@/tutorial/_index.md). The tutorial takes one small part from a model file to G-code in ten steps, with a screenshot of each.

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
