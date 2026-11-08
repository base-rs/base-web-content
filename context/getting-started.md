# Getting Started

This guide explains how to use `base-web-content`.

## Status

The crate is under development. Applications declare the content they accept with `#[derive(Content)]`, with limits on every field, array, and upload, and `Content::body_size_limit` gives the largest body a declaration could need in each format, for a server's `Content-Length` check. The parser that reads `multipart/form-data` and `application/x-www-form-urlencoded` bodies into these declarations, without performing I/O, comes next.
