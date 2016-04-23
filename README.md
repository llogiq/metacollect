# NSA

*Collecting All Your Crate Metadata*

This is a lint that never reports anything, but collects crate metadata like
the composition of types and the call graph.

This can be useful to determine if

* a type has inherent mutability
* a type contains some unsafe other type (e.g. UnsafeCell)
* a function may panic
* a function is pure
* a function is recursive
* a function allocates memory

For now, this is only a work-in-progress proof of concept. There are some open
questions, like

* How to best store the data (currently this writes to tab-separated files, but
the plan is to use [Diesel](http://diesel.rs) to write to an SQLite database.
* How to deal with generics
* How to deal with trait object methods
* How to run the lint from a build script
