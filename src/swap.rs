/// Exchange contents.
pub trait Swap {
	type Other;

	fn swap(&mut self, other: &mut Self::Other) -> bool;
}
