<img src="https://user-images.githubusercontent.com/9434884/43697219-3cb4ef3a-9975-11e8-9a9c-73f4f537442d.png" alt="Sentinel Logo" width="50%">

# Sentinel in Axum

> axum is unique in that it doesn’t have its own bespoke middleware system and instead integrates with tower. This means the ecosystem of tower and tower-http middleware all work with axum.

That is, we can simply reuse [sentinel-tower](https://crates.io/crates/sentinel-tower) in [Axum](https://crates.io/crates/axum).

An example is provided in the `example` directory.