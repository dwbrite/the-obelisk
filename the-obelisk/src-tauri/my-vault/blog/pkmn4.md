---
title: pocket progress 4
aliases:
  - "pocket progress 4"
date: 2015-07-25
tags:
  - pkmn
  - anm_blog
published: true
category: blog
rating: 6
---

I’ve finally got grass working beautifully, and I actually discovered exactly how grass works through a funky glitch in FR/LG.

Here’s how it works: Use cut in the north-most grass in Route 1, take one step into Viridian city, then go back down to the grass. When you step on the empty tiles, the grass particle effects show and you still have a chance of wild encounters. I figured out that rather than having three separate particles for each “piece” of the animation (the stepped on floor, the fluttering grass/leaves, and the part that stays on above your character), it’s all in one particle, that looks like this:

![[grass-rustle.png]]

and changes the depth during the animation. There’s just one frame in which the second image is behind the character which led me to the discovery.

The glitch isn’t listed anywhere I could find easily, which reminds me of another visual glitch in Vermilion City, which is less interesting, but showcases a glaring flaw with the layer system used in the generation 3 games.

![[frlg-layer-glitch.png]]

And lastly, here’s a comparison between what my game looks like now, versus what the real game looks like. Pretty close, right? Forgive the input lag on the left screen (The real game).

![[cmp-frlg.gif]]
