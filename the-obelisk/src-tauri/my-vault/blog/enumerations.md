---
title: rust enums by example
aliases:
  - rust enums by example
date: 2021-06-06
tags:
  - programming
  - rust
  - anm_blog
published: true
category: blog
rating: 7
---

Ever since I started learning Rust about 4 years ago, I've been in love with its enums. You see, Rust's enums aren't strictly enumerations. They're closer to *tagged unions* or *sum types*, which are used to represent variants. Let's take a look at what an enum *is*, and a few cool use-cases for them.

> [!note]- About enums & A comparison to tagged unions in C
> You should already be familiar with [unions](https://en.wikipedia.org/wiki/Union_type) and [traditional enums](https://en.wikipedia.org/wiki/Enumerated_type).
>
> Rust's enums are something of a mix between the two.  
You can read up on the basics in *[the book](https://doc.rust-lang.org/stable/book/ch06-01-defining-an-enum.html?highlight=enum#enum-values)* and/or [Rust by Example](https://doc.rust-lang.org/rust-by-example/custom_types/enum.html).  
That said, let's look at a simple enum in Rust and how it might be implemented in C.

```rust
// example enum in Rust
enum FooBar {
    A(i16),
    B(i32),
    C,
}
```

```c
// rough implementation of FooBar as a tagged union

enum Foo { A, B, C };

union Bar {
    short a;
    int b;
};

struct FooBar {
    Foo tag; // the tag allows us to know which variant our data is.
    Bar data;
};
```

>
> Note that Rust's enums share the same span of memory between its constituent types, so only a small amount of memory is wasted. You can play around with their internal representations in [this playground](https://play.integer32.com/?version=stable&mode=debug&edition=2018&gist=20ae3a196d6a69ad0f2e98b3c01cde1e), or read more in [the nomicon](https://doc.rust-lang.org/nomicon/repr-rust.html).
>

### error handling with variants

Two particularly useful "enums" are `Result` and `Option`.

`Result` gives you a way to represent whether an operation is successful or not, as well as a way to access the data or error of the... result. 👀

`Option` gives you a way to represent whether something exists or not. This is generally used as a replacement for nullable types (which Rust does not have[*](https://doc.rust-lang.org/core/ptr/index.html)).

But what really makes Rust shine is that it forces you to *explicitly* handle enum variants before you can access the underlying data. This is done using [the `match` keyword](https://doc.rust-lang.org/book/ch06-02-match.html). Rust also has [special syntax](https://doc.rust-lang.org/edition-guide/rust-2018/error-handling-and-panics/the-question-mark-operator-for-easier-error-handling.html) for handling Results and Options when you raise issues from the unhappy path.

To start, let's compare how we handle a simple http endpoint with Go's `gorilla/mux` and Rust's `Rocket`.

Rust:
```rust
#[get("/blog/post/<title>")]
fn blog_post(title: &RawStr, state: State<BlogState>) -> Result<Template, CustomError> {
    // `?` is syntactic sugar for propagating errors.
    // url_decode() can fail, and returns a Result when it does.
    // if url_decode() does fail, Rust will return Err(CustomError) for us,
    // (assuming that we've provided a type conversion with From)

    let key = title.url_decode()?; // <- error propagation
    let post = state.title_map.get(key.as_str())?; // <- more error propagation

    let c = Context {
        title: "Devin's Blog".to_string(),
        posts: vec![post.clone()],
    };

    Ok(Template::render("blog", &c))
}
```

Go:
```go
// routed from "/blog/post/{title}"
func (bs BlogState) ServeBlogPost(writer http.ResponseWriter, request *http.Request) {
    key := mux.Vars(request)["title"]
    post, ok := bs.TitleMap[key]
    if !ok {
        http.Error(writer, "blog post not found", http.StatusBadRequest)
		return
    }

	ctx := Context{
        title: "Devin's Blog",
        posts: make([]BlogPost, post),
	}

	err := bs.Template.Execute(writer, ctx)
	if err != nil {
        http.Error(writer, err.Error(), http.StatusInternalServerError)
	}
}
```

By having data embedded into variants, we can represent whether an operation is successful or not. Potential errors can be propagated, transformed, and handled without mucking up your happy path.

Very cool. 😎

Let's go deeper.

### heap allocation and dynamic dispatch

Imagine you're writing an audio system for a game. You have a directed acyclic graph and you need a way to represent the nodes in this graph. A node can be an input (sine wave, mp3), effect (pan, mix), or output (speakers, a file, visualizer).

What all nodes have in common is one function: `process(inputs, outputs)`. Let's call this common behaviour the `AudioNode` interface (or *trait*).

So our audio graph looks something like `Graph<AudioNode>`.  
In practice then, each node in the graph is *dynamically sized* and must be heap allocated. To *perform* that heap allocation in Rust, our nodes must be wrapped in a smart pointer: `Box<dyn AudioNode>`.

Then the `process(..)` function needs to be [dynamically dispatched](https://en.wikipedia.org/wiki/Dynamic_dispatch).

All this results in significant overhead with multiple vtable accesses, and more importantly: *indirection which prevents compiler optimization.*

Keep in mind `process(..)` is called *multiple thousands of times per second*.

---

But we can improve that with enums:

```rust
// We can put all of our node variants inside of an enum.
// This allows our data to stay on the stack,
// improving cache locality and eschewing a heap allocation.
// Note however, that we *must* be careful with variant sizes,
// as any NodeVariant will take as much space on the stack as the largest variant.
pub enum NodeVariant {
    CpalOut(CpalMonoSink),
    SineIn(Sine),
    SquareIn(Square),
    SumFX(Sum),
    SlewLimFX(SlewLimiter),
}

// We end up manually implementing dynamic dispatch,
// but in a way which enables compiler optimization and reduces indirection.
impl AudioNode for NodeVariant {
    fn process(&mut self, inputs: &[Input], output: &mut [Buffer]) {
        match self {
            NodeVariant::CpalOut(s) => s.process(inputs, output),
            NodeVariant::SineIn(s) => s.process(inputs, output),
            NodeVariant::SquareIn(s) => s.process(inputs, output),
            NodeVariant::SumFX(s) => s.process(inputs, output),
            NodeVariant::SlewLimFX(s) => s.process(inputs, output),
        }
    }
}
```

In my project I'm getting *up to 10%* better performance. Not at all laughable in audio programming. You can remove a lot of boilerplate here with the `impl-enum` or `enum_dispatch` crates ([see enum_dispatch benchmarks](https://docs.rs/enum_dispatch/0.3.6/enum_dispatch/#the-benchmarks)).

Very, very cool. 😎

### message passing

Imagine you're writing a music player. Your UI has controls for play/pause, seek, skip, etc. These inputs can come from different places - like [dbus](https://specifications.freedesktop.org/mpris-spec/latest/#Interfaces), hotkeys, or simple UI interactions.

We've just run into an ideal use-case for *MPSC (multi-producer/single-consumer) channels*! An *MPSC channel* is simply an atomic queue that can only be accessed through its *producers* and *consumers*.

Whenever one of the aforementioned controls are triggered, we can send a message through an MPSC channel to *control* playback. With that out of the way, we need to determine what data to send.

Let's look at some potential *Java-esque* solutions. Normal enums won't work because *some* of our controls like `Seek(timestamp)` have associated data. Maybe a class with an enum field, plus fields for each type of associated data would work? Or a string?

It's an oddly gnarly problem to solve.

Fortunately for us, Rust's enums make this easy. This is part of what makes multithreading so nice in Rust.

### closing thoughts

The last pattern using enums that I'd like to shine some light on is the *finite state machine*. Plenty of others have written about [state machines in Rust](https://hoverbear.org/blog/rust-state-machine-pattern/) before, so I won't [reiterate](https://blog.yoshuawuyts.com/state-machines/) on that.

Hopefully you've learned something new about rust's enums - whether you've never seen Rust before, or you're a Rust veteran. If you have any questions, feedback, or flattery, you can find my contact info on [my résumé](https://dwbrite.com/resume).

---

Speaking of résumés, I'm looking for work right now!

I'm a generalist software developer with a specialization in backend / web architecture. I've spent the last nine years honing the craft in my free time, and I'd really like to get my foot in the door professionally. Send me an email if you know of any internships, contract positions, or full-time employment that I might be a good fit for!

\- burdock
