---
title: lightboard follow-up (aka lightboard v2)
aliases:
  - "lightboard follow-up (aka lightboard v2)"
date: 2020-06-10
tags:
  - lightboard
  - etc
  - anm_blog
published: true
category: blog
rating: 3
---
Nearly five years ago I finished a "lightboard" project consisting of a WS2812 LED strip, a sheet of plexiglas, and an [adafruit trinket.](https://www.adafruit.com/product/1501) I never did write a blog post about it, but now is as good a time as ever.

Originally, the 48"x24"x1/8" acrylic sheet was mounted with a 1/4" wide plastic J channel screwed into drywall. The top of the sheet was kept in place with a nail at each corner, allowing the acrylic to slide out easily for maintenance. This mounting solution was not quite good enough, and daily use wore down the drywall until the board could no longer stand.

A few years later, I resolved to make a better lightboard, an *advanced* lightboard! I built plywood shelves for an unrelated project, and used that to mount the next revision of the lightboard. The new revision uses side-mounted WS2812 LEDs, which can fit perfectly in a 1/8" aluminum J channel.

![[j-channel.jpg]]

With the mounting hardware set up, it was time to work on the electronics. I had spent a good chunk of my free time in the past year writing [firmware](https://github.com/dwbrite/teensy-k20dx256) in [Rust](https://github.com/dwbrite/keebo-firmware) for the [teensy 3.2](). I hoped I could use this firmware to build the lightboard's interface, so I added SPI functionality to the teensy, and... it wasn't fast enough! Turns out, assembly is essential for firmware development. And I *really* did not feel like learning assembly at the time.

So, I resigned myself to learning C++ and writing [the software](https://github.com/dwbrite/lightboard-controls) in that with the help of Arduino libraries.

(holy heck, this is a long post, sorry)

Onto the interface! The plan is to use three rotary encoders and a small OLED screen. Then I'll plop these into a 3D printed enclosure (which I'll figure out later). For now, I've mounted the prototype to some cardboard!~

![[cardboard-prototype.jpg]]

![[cardboard-back.jpg]]

The three rotary encoders will control things like the "mode" of the lightboard - on, off, rainbow cycle, etc - parameters of different modes, and resets for those parameters. I haven't perfectly defined the interface yet, but I have plenty of options. Right now I'm thinking each mode will have three parameters, and the mode can be changed by depressing the first knob while turning.

I'm going to end this post here and leave the software for another post.

But first... A celebration!

I've recently graduated with an A.S. in Computer Information Systems, and I've been accepted into UMass Amherst for a B.S. in Mathematics! 🎉

I'm still not sure about what I'll do in the future, but at least I've accomplished a few things, eh?

Ciao!~
