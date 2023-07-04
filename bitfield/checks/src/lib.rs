pub struct SevenMod8;

pub struct EightMod8;

pub trait TotalSizeIsMultipleOfEightBits {}

impl TotalSizeIsMultipleOfEightBits for EightMod8 {}

pub struct False;

pub struct True;

pub trait DiscriminantInRange {
    fn method(&self);
}

impl DiscriminantInRange for True {
    fn method(&self) {}
}
