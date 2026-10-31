+++
title = "OpenChuckCAM user guide"
sort_by = "weight"
+++

OpenChuckCAM turns a 3D model into G-code for 4-axis rotary machining: the work is held at one end in a chuck and turned about its length while the cutter moves along and around it. The first machine it supports is the Makera Z1.

This guide is for people who want to make parts with the program, including people who have not used [CAM](@/glossary.md#cam) software before. It explains each term the first time it is used and collects them in the [glossary](@/glossary.md).

{% warning() %}
Generated G-code must be checked by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. The program is in early development, and very little of its output has been run on a real machine. Read *Checking G-code before you run it* before you cut anything.
{% end %}

## Where to start

<dl>
<dt>New to the program</dt>
<dd>Read <em>Install and first run</em>, then work through <a href="@/tutorial/_index.md">Your first project</a>. The tutorial takes one small part from a model file to G-code in ten steps, with a screenshot of each.</dd>
<dt>New to CAM or to 4-axis work</dt>
<dd>The <em>Concepts</em> pages explain what a rotary axis does, what stock and a held region are, and why a job is usually cut in two passes, roughing and then finishing.</dd>
<dt>Ready to run a program on the Makera Z1</dt>
<dd>Follow <em>Air cut on the Makera Z1</em> and the <em>Safe-practice checklist</em>. An air cut runs the program with no tool and no material, so you can watch the moves before anything can be damaged.</dd>
<dt>Looking for a setting or a word</dt>
<dd>Use the search box, the <a href="@/glossary.md">glossary</a>, or the <em>Reference</em> pages.</dd>
</dl>

## What the program does

| Step in the program | What you do there |
| --- | --- |
| Model | Import an STL or STEP file, check it, and place it on the rotary axis. |
| Setup | Choose the machine, describe the stock and how the work is held. |
| Operations | Choose tools and add the cutting operations, from roughing to finishing. |
| Simulate | Generate the toolpaths, watch the cut, and compare the result with the model. Then post and export the G-code and a setup sheet. |

## About this guide

This guide describes the development version of OpenChuckCAM. Each release has its own copy; use **Version** at the top of the page to switch. The guide is part of the project's source and is licensed AGPL-3.0-or-later. To report a mistake in it, open an issue in the project's repository.
