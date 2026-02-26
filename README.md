# lvquick
lvquick (lvq) is a transactional LVM2 wrapper. It replaces “fire-and-forget” storage commands with a Plan → Verify → Confirm → Execute workflow, featuring immutable execution plans, state journaling, drift detection, and safe recovery. Built in Rust with formally verified size math, it makes high-risk LVM operations boring and predictable.
