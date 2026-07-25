---
title: staring into the void
aliases:
  - "staring into the void"
date: 2021-03-18
tags:
  - void
  - gamedev
  - anm_blog
published: true
category: blog
rating: 6
---

Long ago I went to a 48 hour game jam at Becker University. It was the global game jam and the theme was "what do we do now?" At the time I had already been working on a 2D game project on top of a javafx canvas, so I copied the text rendering code and said "let's make a text adventure!"

It was a fun little project, but little did we know the *dangers* of such a thing. You see, we were in ye olde java 7 times[^1], fresh out of our second year of computer science in high school. We had even struggled to represent branching trees of text that could loop back upon themselves. Perhaps better known as *directed graphs*.

Our "*solution*" was to write a [big ol' switch statement.](https://github.com/dwbrite/void_2015/blob/6da801eb44577ec7dc2601d8d5f6dc0d827a05d4/src/GameState/Story.java) <sup>Teehee 😇 what's a stack overflow?</sup>

Text was represented as strings with special tokens to change how fast text was printed. For example, `"\^"` meant the following text should print quickly, and `"\#"` meant that text should print slow. This has made many people very angry and has been widely regarded as a bad move.

![[void_rust.webm]]

A few years later, I tried to run the game out on Linux, and, *surprise*! 🎉  
Audio doesn't play, the logic thread crashes and burns, and you're left with an unresponsive window. Write once, run anywhere, eh?[^2]

<style>
@keyframes wiggle {
    0% { transform: translateY(0 px); }
    1.5% { transform: translateY(-4 px); }
    3% { transform: translateY(0 px); }
}

.wiggle span {
    display: inline-block;
}

.wiggle span:nth-child(1) { animation: wiggle 4 s infinite 0.0 s; }
.wiggle span:nth-child(2) { animation: wiggle 4 s infinite 0.07 s; }
.wiggle span:nth-child(3) { animation: wiggle 4 s infinite 0.14 s; }
.wiggle span:nth-child(4) { animation: wiggle 4 s infinite 0.21 s; }
.wiggle span:nth-child(5) { animation: wiggle 4 s infinite 0.28 s; }
.wiggle span:nth-child(6) { animation: wiggle 4 s infinite 0.35 s; }
.wiggle span:nth-child(7) { animation: wiggle 4 s infinite 0.42 s; }
.wiggle span:nth-child(8) { animation: wiggle 4 s infinite 0.49 s; }
.wiggle span:nth-child(9) { animation: wiggle 4 s infinite 0.56 s; }
.wiggle span:nth-child(10) { animation: wiggle 4 s infinite 0.63 s; }
.wiggle span:nth-child(11) { animation: wiggle 4 s infinite 0.70 s; }
.wiggle span:nth-child(12) { animation: wiggle 4 s infinite 0.77 s; }
.wiggle span:nth-child(13) { animation: wiggle 4 s infinite 0.84 s; }

@keyframes typewrite {
    0% { opacity: 0%; }
    9.99% { opacity: 0%; }
    10% { opacity: 100%; }
    60% { opacity: 100%; }
    65% { opacity: 0%; }
    100% { opacity: 0%; }
}

.typewrite > span { opacity 0%; }

.typewrite span:nth-child(1) { animation: typewrite 8 s infinite 0.0 s; }
.typewrite span:nth-child(2) { animation: typewrite 8 s infinite 0.25 s; }
.typewrite span:nth-child(3) { animation: typewrite 8 s infinite 0.5 s; }
.typewrite span:nth-child(4) { animation: typewrite 8 s infinite 0.75 s; }
.typewrite span:nth-child(5) { animation: typewrite 8 s infinite 1.0 s; }
.typewrite span:nth-child(6) { animation: typewrite 8 s infinite 1.25 s; }
.typewrite span:nth-child(7) { animation: typewrite 8 s infinite 1.5 s; }
</style>

<p>
So 6 years after <i>that</i>, I finally resolved to finish that game <i>the right way.</i>
The core idea of void is to have
text that is <i>engaging</i>. Sometimes you want
<span class="wiggle">
    <span>t</span><span>e</span><span>x</span><span>t</span>
    <span>t</span><span>o</span>
    <span>w</span><span>i</span><span>g</span><span>g</span><span>l</span><span>e</span>,
</span>
or type-write
<span class="typewrite">
<span>s</span><span>l</span><span>o</span><span>w</span><span>l</span><span>y</span><span>,</span>
</span>
or any other of the infinite possibilities to add <i>character</i> to text.
Which makes <em>any</em> markup language a natural choice for text representation.
</p>

And with that I've started using xml to represent my game text. This required parsing my xml and turning it into data structures stored in [bincode](https://github.com/bincode-org/bincode) files for later use. The last part to navigate then, is branching storylines. I initially decided that this would also be done in xml, so long as the logic isn't much more complicated than checking booleans - but part of me is thinking that maybe these logic checks should be done in Rust.

Anyway, that's all for today! Hopefully I won't get too distracted with other projects in the near future

---

[^1]: Technically java 8 had just come out the year before, but we were inexperienced - we didn't even know what a lambda *was*, let alone how to use it. Frankly, I even thought writing code in a legacy style was considered good practice because it was "backwards compatible" 🤦

[^2]: As long as you're not on linux. And while we're at it, even if you successfully create a cross-platform abstraction layer, you'd need to pack all the abstractions into one distributable. *Or* create multiple distributables.
