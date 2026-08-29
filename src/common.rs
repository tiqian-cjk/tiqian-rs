#[cfg(all(feature = "collections-hashbrown", feature = "collections-std"))]
compile_error!("only one collection implementation may be enabled");

#[cfg(not(any(feature = "collections-hashbrown", feature = "collections-std")))]
compile_error!("a collection implementation must be enabled");

#[cfg(feature = "collections-hashbrown")]
pub use hashbrown::{HashMap, HashSet};

#[cfg(feature = "collections-std")]
pub use std::collections::{HashMap, HashSet};
