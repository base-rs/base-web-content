# `base-web-content`

Declarative, limited parsing of web request content, such as forms, without I/O, for Rust.

Applications declare the content they accept, with limits on every field and upload, so that a server never parses a body without knowing its limits. The crate is fed bytes and does no I/O. It is under development and has no public API yet.

## Usage

See the [Getting Started](context/getting-started.md) guide.

## Releases

See [releases.md](releases.md).

## See Also

- [Protocol::Content](https://github.com/socketry/protocol-content), the Ruby gem this crate follows.
- [`base-web-multipart`](https://github.com/base-rs/base-web-multipart), which parses the multipart bodies of forms.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/base-web-content).

Install the [Bake launcher](https://github.com/base-rs/bake) and run `cargo bake test`. Use `cargo bake test:coverage --all-targets true` for the shared source-region coverage gate.

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover relevant guides, follow repository guidance in `.agents/`, and apply skills under `.agents/skills/`.

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.
