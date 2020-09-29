# Overview
This crate provides an extractor for working with CBOR.
It closely mirrors the API for JSON extraction within Actix-Web, and in fact borrows most of it's
code from Actix-Web.

# Example
```rust
use actix_cbor::Cbor;

struct User {
    name: String,
}
struct Greeting {
    inner: String,
}

#[get("/users/hello")]
pub async fn greet_user(user: Cbor<User>) -> Cbor<Greeting> {
    let name: &str = &user.name;
    let inner: String = format!("Hello {}!", name);
    Cbor(Greeting { inner })
}
```

# Contributing
If you have a bug report or feature request, create a new GitHub issue.

Pull requests are welcome.