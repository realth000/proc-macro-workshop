// Use this trait to "attach" getters and setters to #[bitfield].
//
// When getting and setting values, just pass in its offset in `data` and its length.
pub trait BitParse {
    type Data: AsRef<[u8]> + AsMut<[u8]> + Sized;

    fn get_data(&self) -> &Self::Data;

    fn get_mut_data(&mut self) -> &mut Self::Data;

    #[allow(
        clippy::missing_errors_doc,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    // length is how may bits to set.
    fn set_bits_value(
        &mut self,
        offset_bits: usize,
        allowed_length_bits: usize,
        value: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let d = self.get_mut_data();
        let data = d.as_mut();

        // Value's length in bits.
        let mut value_length = 64 - value.leading_zeros() as usize;
        // How many bits are allowed to save this field in this byte.
        let mut allowed_length = allowed_length_bits;

        // left side is in bytes, right side offset and length is in bits.
        if allowed_length < value_length {
            return Err(Box::<dyn std::error::Error>::from(format!(
                "overflow when setting value: value length is {value_length} bits but only {allowed_length} bits allowed",
            )));
        }

        let mut outer = offset_bits / 8;
        let mut inner = offset_bits % 8;
        let mut bits_empty = 8 - inner;

        loop {
            if bits_empty >= value_length {
                // `value` can be saved in current Bytes.
                //
                //        byte (outer)
                //     ╟───────────────╫
                //     ║▒ ▒ ▒ ▒ ▒ ▒ ▒ ▒╫
                //     ║  ^ ------- ^  ║
                //      inner    allowed_length
                //
                // If allowed_length is no more than 8, maybe value can be stored in this byte and
                // there still reset bits for the next field.
                let bit_mask = if 8 - inner > allowed_length {
                    // There are some bits for the next field.
                    ((0x00FF << (8 - inner)) as u8) | (0xFF >> (inner + allowed_length))
                } else {
                    // No bits in the tail of this byte is remained for the next field
                    (0x00FF << (8 - inner)) as u8
                };

                // Clear all bits that belongs to current field.
                // The head `inner` bits are used by the former field and should not be modified.
                let mut v = data[outer] & (bit_mask);

                // Update value.
                //
                // !bit_mask: 00111000  2
                // value1:    10111100 << 3
                // value2:    00000011 << 3
                v |= (!bit_mask) & (((value as u16) << (!bit_mask).trailing_zeros()) as u8);
                data[outer] = v;
                value_length = 0;
            } else {
                // First `bits_empty` bits save in current byte.
                let bit_mask = (0x00FF << (8 - inner)) as u8;

                // Clear all bits that belongs to current field.
                // The head `inner` bits are used by the former field and should not be modified.
                let mut v = data[outer] & bit_mask;

                // Update value.
                v |= (!bit_mask) & ((value >> (value_length - (8 - inner))) as u8);

                data[outer] = v;

                // Saved 8-inner bytes in this loop so value left length is (8 -inner) bits smaller.
                value_length -= 8 - inner;
                allowed_length -= 8 - inner;

                // Inner should be zero after the first loop, because only the first byte we start
                // to store value can be also used by former field.
                // Also, all 8 bytes in the next byte is remained for current field.
                inner = 0;
                bits_empty = 8;
                outer += 1;
            }

            if value_length == 0 {
                break;
            }
        }

        Ok(())
    }

    #[allow(
        clippy::missing_errors_doc,
        clippy::cast_lossless,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    fn get_bits_value(&self, offset_bits: usize, length_bits: usize) -> u64 {
        let data = self.get_data().as_ref();
        let mut ret: u64 = 0;
        let mut length = length_bits;

        let mut outer = offset_bits / 8;
        let mut inner = offset_bits % 8;

        loop {
            if length + inner <= 8 {
                let bit_mask = if 8 - inner > length {
                    // (0xFF << (8 - inner)) | (0xFF >> (inner + length))
                    ((0x00FF << (8 - inner)) as u8) | (0xFF >> (inner + length))
                } else {
                    (0x00FF << (8 - inner)) as u8
                };
                // All value is stored in current bit;
                ret |= ((data[outer] & !bit_mask) as u64) >> (!bit_mask).trailing_zeros();
                // panic!(
                //     "result!!! {}, {} {}",
                //     !bit_mask,
                //     (data[outer] | !bit_mask),
                //     ret
                // );
                length = 0;
            } else {
                let bit_mask = (0x00FF << (8 - inner)) as u8;

                ret |= ((data[outer] & !bit_mask) as u64) << (length - (8 - inner));

                // The following code: `length -= 8 - inner;`
                // equals to ` length = length - (8 - inner)`.
                length -= 8 - inner;
                inner = 0;
                outer += 1;
            }

            if length == 0 {
                break;
            }
        }
        ret
    }
}
