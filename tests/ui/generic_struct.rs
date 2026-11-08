use base_web_content::Content;

#[derive(Content)]
struct Wrapper<T> {
    value: T,
}

fn main() {}
