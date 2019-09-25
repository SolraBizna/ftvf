`ftvf` is a crate for carrying out game logic the One True Way: Fixed
Tickrate, Variable Framerate. By having your game logic in strictly
fungible ticks, rather than having it vary based on framerate, you gain
many advantages:

- **Repeatability**: the same inputs will have the same outputs, period.
- **Framerate independence**: no issues like Quake had where your exact
jump height depends on how fast your computer is.
- **Satisfaction**: knowing that you made the morally correct choice. :)

To get started, add `ftvf` to your dependencies in `Cargo.toml`:

```toml
ftvf = "0.5"
```

then initialize yourself a [`Metronome`](struct.Metronome.html):

```rust
let mut metronome = Metronome::new(RealtimeNowSource::new(),
                                   (30, 1), // want 30 ticks per 1 second
                                   5); // accept being up to 5 ticks behind
```

And then your game loop looks like this:

```rust
while !world.should_quit() {
  world.handle_input();
  // call `sample` once per batch. not zero times, not two or more times!
  metronome.sample();
  while let Some(status) = metronome.status(Mode::UnlimitedFrames) {
    match status {
      Status::Tick => world.perform_tick(),
      Status::Frame{phase} => world.render(phase),
      Status::TimeWentBackwards
        => eprintln!("Warning: time flowed backwards!"),
      Status::TicksLost(n)
        => eprintln!("Warning: we're too slow, lost {} ticks!", n),
      // No special handling or warning message is needed for Rollover. In
      // practice, it will never be seen.
      Status::Rollover => (),
      // Mode::UnlimitedFrames never returns Idle, but other modes can, and
      // this is the way it should be handled.
      Status::Idle => metronome.sleep_until_next_tick(),
    }
  }
}
```

Your logic ticks operate in discrete, fixed time intervals. Then, when it
comes time to render, you render a frame which represents time some portion
of the way between two ticks, represented by its `phase`. Your rendering
process should render an interpolated state between the previous tick and
the current tick, based on the value of `phase`. Simple example:

```rust
self.render_at(self.previous_position
               + (self.current_position - self.previous_position) * phase);
```

# License

`ftvf` is distributed under the zlib license. The complete text is as
follows:

> Copyright (c) 2019, Solra Bizna
> 
> This software is provided "as-is", without any express or implied
> warranty. In no event will the author be held liable for any damages
> arising from the use of this software.
> 
> Permission is granted to anyone to use this software for any purpose,
> including commercial applications, and to alter it and redistribute it
> freely, subject to the following restrictions:
> 
> 1. The origin of this software must not be misrepresented; you must not
> claim that you wrote the original software. If you use this software in a
> product, an acknowledgement in the product documentation would be
> appreciated but is not required.
> 2. Altered source versions must be plainly marked as such, and must not
> be misrepresented as being the original software.
> 3. This notice may not be removed or altered from any source
> distribution.
