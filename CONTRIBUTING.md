# Contributing guidelines

## Contributing code

To contribute a feature or bugfix,
create a pull request directly.
Consider creating an issue first to assess its feasibility
if the change is large.

### Commit messages

This project uses [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/).

The scope in commit messages is REQUIRED for feat, fix and perf changes
that semantically affect reverse dependencies (i.e. except tests and benches).
It MUST be a top-level module name or
`<crate>/<module>` for changes exclusive to a non-main crate (such as `codegen`).
In the case where multiple modules are affected,
use the module where the change is mainly intended to affect.

### Commit validation

Create the file `.git/hooks/pre-commit` with the following contents
and `chmod +x` it:

```bash
#!/bin/bash
test -z "$SKIP_COMMIT_CHECKS" || exit 0
typos || exit 1 # cargo install typos
cargo fmt --all -- --check || exit 1
cargo clippy --release --all || exit 1
cargo clippy --release --tests --all || exit 1
cargo test --all || exit 1
```

### Code style

#### Generics

dynec uses generics very extensively.
Code quickly becomes confusing when there are multiple type parameters in scope.
Naming type parameters follows the following conventions:

- If the type parameter has no specific bounds
  and there are no more than two type parameters in scope,
  the dummy names `T` and `U` MAY be used.
- If the type parameter is defined in the scope of a key-value collection,
  i.e. the item at which the type parameters are defined
  corresponds exclusively to exactly one key-value item,
  the names `K` and `V` MAY be used for key and value types on the same collections.
  `K` or `V` SHOULD NOT be used alone without the other one or on different collections.
- `F` and `I` MAY be used to represent closure and iterator types
  only if there is no more than one generic function/iterator type in scope.
- Domain-specific acronyms: The following type parameters are ALLOWED only if
  they describe the following dynec-specific concepts:
  - `A`: Archetype
  - `C`: Component (Simple or Isotope)
  - `D`: Discriminant (for isotope components)
  - `E`: Implements `entity::Ref` (NOT `entity::Raw`)
- Otherwise, the full name SHOULD be described in PascalCase directly
  if it does not collide with the name of a type/trait used in this project
  (regardless whether it is *currently* imported).
- In the case of name collision, a `T` MAY be appended to its name.

#### Imports

- The `Result` type MUST NOT be aliased in the main crate.
  However, importing `syn::Result` is RECOMMENDED in the codegen crate.
- Traits SHOULD be imported `as _`
  if the imported trait identifier is not directly used
  (if the module only uses the imported methods in the trait).
- Imports from the standard library MUST prefer `std` over `core`.
