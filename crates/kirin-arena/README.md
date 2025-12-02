# Kirin Arena

This is a similar implementation as id-arena crate, but with the following extra considerations:

- the arena items can be marked as deleted
- the arena can clean up its memory by going through the deleted items
- the arena can accept new items with a given new ID, and the uniqueness can be verified later
- only bare-bone `u32` is used as ID for simplicity, we will create the ID wrapper as `SSAValue` etc.

this is essentially a GC but very light-weight designed for the compiler rewrites,
instead of marking the lifetime of an IR object by lifetime, the deletion is informed
by the rewrite engine.
