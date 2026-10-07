//! DF animation resources and action playback state.

use armv4t_emu::Memory;

use super::super::NicaiMachine;

struct Frame {
    duration: i16,
    parts: Vec<[i16; 5]>,
}

struct Animation {
    images: Vec<Vec<u8>>,
    rectangles: Vec<[i16; 5]>,
    actions: Vec<Vec<Frame>>,
}

fn parse_animation(bytes: &[u8]) -> Option<Animation> {
    struct Reader<'a> {
        bytes: &'a [u8],
        position: usize,
    }
    impl Reader<'_> {
        fn take(&mut self, size: usize) -> Option<&[u8]> {
            let end = self.position.checked_add(size)?;
            let value = self.bytes.get(self.position..end)?;
            self.position = end;
            Some(value)
        }
        fn integer(&mut self) -> Option<i16> {
            let value = i32::from_le_bytes(self.take(4)?.try_into().ok()?);
            i16::try_from(value).ok()
        }
        fn count(&mut self) -> Option<usize> {
            usize::try_from(self.integer()?).ok().filter(|n| *n <= 4096)
        }
    }
    let mut reader = Reader { bytes, position: 0 };
    let mut images = Vec::new();
    for _ in 0..reader.count()? {
        let length = usize::from(*reader.take(1)?.first()?);
        images.push(reader.take(length)?.to_vec());
    }
    let mut rectangles = Vec::new();
    for _ in 0..reader.count()? {
        let mut rectangle = [0; 5];
        for value in &mut rectangle {
            *value = reader.integer()?;
        }
        if rectangle[2] < rectangle[0]
            || rectangle[3] < rectangle[1]
            || rectangle[2].checked_sub(rectangle[0]).is_none()
            || rectangle[3].checked_sub(rectangle[1]).is_none()
            || usize::try_from(rectangle[4]).ok()? >= images.len()
        {
            return None;
        }
        rectangles.push(rectangle);
    }
    let mut actions = Vec::new();
    let mut total_parts = 0usize;
    for _ in 0..reader.count()? {
        let mut frames = Vec::new();
        for _ in 0..reader.count()? {
            let duration = reader.integer()?;
            let count = reader.count()?;
            total_parts = total_parts.checked_add(count)?;
            if total_parts > 65536 {
                return None;
            }
            let mut parts = Vec::new();
            for _ in 0..count {
                let mut part = [0; 5];
                for value in &mut part {
                    *value = reader.integer()?;
                }
                if usize::try_from(part[0]).ok()? >= rectangles.len() {
                    return None;
                }
                parts.push(part);
            }
            frames.push(Frame { duration, parts });
        }
        actions.push(frames);
    }
    Some(Animation {
        images,
        rectangles,
        actions,
    })
}

impl NicaiMachine {
    pub(crate) fn handle_actor_method(&mut self, offset: u32) {
        let actor = self.register(0);
        if actor == 0 {
            self.set_result(0);
            return;
        }
        let head = self.memory.r32(actor + 12);
        let action = self.memory.r16(actor + 6) as i16;
        let count = if head != 0 {
            self.memory.r16(head + 8) as i16
        } else {
            0
        };
        let action_pointer = if action >= 0 && action < count {
            self.memory.r32(head + 12) + action as u32 * 8
        } else {
            0
        };
        let result = match offset {
            0x10 => self.load_actor(actor, self.register(1), self.register(2)),
            0x14 | 0x18 if action_pointer != 0 => {
                let current = self.memory.r16(actor + 8) as u32;
                if current < self.memory.r16(action_pointer) as u32 {
                    let frame = self.memory.r32(action_pointer + 4) + current * 8;
                    let parts = self.memory.r32(frame + 4);
                    let library = self.memory.r32(head + 16);
                    let mut target = self.memory.r32(library + 4);
                    if target == 0 {
                        target = super::super::SCREEN_IMAGE_STRUCT;
                    }
                    let origin_x = if offset == 0x18 {
                        self.register(1) as i16 as i32
                    } else {
                        0
                    };
                    let origin_y = if offset == 0x18 {
                        self.register(2) as i16 as i32
                    } else {
                        0
                    };
                    for index in 0..self.memory.r16(frame + 2) as u32 {
                        let part = parts + index * 10;
                        let rectangle =
                            self.memory.r32(head + 4) + self.memory.r16(part) as u32 * 10;
                        let image_slot = self.memory.r16(rectangle + 8) as u32;
                        let image_slots = self.memory.r32(head);
                        let image_index = self.memory.r16(image_slots + image_slot * 2) as u32;
                        if image_index >= self.memory.r16(library + 20) as u32 {
                            continue;
                        }
                        let images = self.memory.r32(library + 16);
                        let image = self.memory.r32(images + image_index * 4);
                        let values: Vec<i32> = (0..4)
                            .map(|field| self.memory.r16(rectangle + field * 2) as i16 as i32)
                            .collect();
                        let x = self.memory.r16(actor) as i16 as i32
                            + self.memory.r16(part + 2) as i16 as i32
                            - origin_x;
                        let y = self.memory.r16(actor + 2) as i16 as i32
                            + self.memory.r16(part + 4) as i16 as i32
                            - origin_y;
                        self.blit_image(
                            target, image, values[0], values[1], values[2], values[3], x, y, true,
                        );
                    }
                }
                0
            }
            0x20 => {
                let requested = self.register(1) as i16;
                if requested >= 0 && requested < count && requested != action {
                    self.memory.w16(actor + 6, requested as u16);
                    self.memory.w16(actor + 8, 0);
                    self.memory.w16(actor + 10, 0);
                }
                actor
            }
            0x24 if action_pointer != 0 => self.memory.r16(action_pointer) as u32,
            0x1c if action_pointer != 0 => {
                let frames = self.memory.r16(action_pointer) as u32;
                let current = self.memory.r16(actor + 8) as u32;
                if frames == 0 || current >= frames {
                    0
                } else {
                    let pointer = self.memory.r32(action_pointer + 4);
                    let elapsed = self.memory.r16(actor + 10).saturating_add(1);
                    self.memory.w16(actor + 10, elapsed);
                    let duration = self.memory.r16(pointer + (current + 1).min(frames - 1) * 8);
                    if elapsed >= duration {
                        let next = if current + 1 >= frames {
                            0
                        } else {
                            current + 1
                        };
                        self.memory.w16(actor + 8, next as u16);
                        if next == 0 {
                            self.memory.w16(actor + 10, 0);
                        }
                        next
                    } else {
                        duration as u32
                    }
                }
            }
            _ => 0,
        };
        self.set_result(result);
    }

    fn load_actor(&mut self, actor: u32, library: u32, name: u32) -> u32 {
        let source = self.resource_by_name(name);
        let data = self.decode_resource_stream(source);
        let size = self.allocation_size(data).unwrap_or(0);
        let bytes: Vec<_> = (0..size)
            .map(|offset| self.memory.r8(data + offset))
            .collect();
        let parsed = parse_animation(&bytes);
        self.deallocate(data);
        let Some(animation) = parsed else {
            self.memory.w32(actor + 12, 0);
            return 0;
        };
        let head = self.allocate(20);
        let images = self.allocate((animation.images.len() as u32 * 2).max(2));
        for (index, mut name) in animation.images.into_iter().enumerate() {
            name.push(0);
            let text = self.allocate(name.len() as u32);
            self.memory.write_bytes(text, &name);
            let cpu = self.cpu;
            self.cpu.reg_set(armv4t_emu::Mode::User, 0, library);
            self.cpu.reg_set(armv4t_emu::Mode::User, 1, text);
            self.handle_picture_library_method(0x1c);
            let image = self.register(0);
            self.cpu = cpu;
            self.deallocate(text);
            self.memory.w16(images + index as u32 * 2, image as u16);
        }
        let rectangles = self.allocate((animation.rectangles.len() as u32 * 10).max(10));
        for (index, rectangle) in animation.rectangles.into_iter().enumerate() {
            let values = [
                rectangle[0],
                rectangle[1],
                rectangle[2] - rectangle[0],
                rectangle[3] - rectangle[1],
                rectangle[4],
            ];
            for (field, value) in values.into_iter().enumerate() {
                self.memory.w16(
                    rectangles + index as u32 * 10 + field as u32 * 2,
                    value as u16,
                );
            }
        }
        let count = animation.actions.len();
        let actions = self.allocate((count as u32 * 8).max(8));
        for (index, frames) in animation.actions.into_iter().enumerate() {
            let action = actions + index as u32 * 8;
            let table = self.allocate((frames.len() as u32 * 8).max(8));
            self.memory.w16(action, frames.len() as u16);
            self.memory.w32(action + 4, table);
            for (frame_index, frame) in frames.into_iter().enumerate() {
                let address = table + frame_index as u32 * 8;
                let parts = self.allocate((frame.parts.len() as u32 * 10).max(10));
                self.memory.w16(address, frame.duration as u16);
                self.memory.w16(address + 2, frame.parts.len() as u16);
                self.memory.w32(address + 4, parts);
                for (part_index, values) in frame.parts.into_iter().enumerate() {
                    for (field, value) in values.into_iter().enumerate() {
                        self.memory.w16(
                            parts + part_index as u32 * 10 + field as u32 * 2,
                            value as u16,
                        );
                    }
                }
            }
        }
        self.memory.w32(head, images);
        self.memory.w32(head + 4, rectangles);
        self.memory.w16(head + 8, count as u16);
        self.memory.w32(head + 12, actions);
        self.memory.w32(head + 16, library);
        self.memory.w32(actor + 12, head);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::parse_animation;

    #[test]
    fn animation_preserves_cumulative_frame_times_and_rejects_bad_rectangles() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(b"\x05a.gif");
        for value in [
            1i32, 0, 0, 4, 5, 0, 1, 2, 0, 1, 0, 2, 3, 0, 0, 4, 1, 0, 2, 3, 0, 0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let animation = parse_animation(&bytes).unwrap();
        assert_eq!(animation.images[0], b"a.gif");
        assert_eq!(animation.actions[0].len(), 2);
        assert_eq!(animation.actions[0][1].duration, 4);
        assert_eq!(animation.actions[0][0].parts[0], [0, 2, 3, 0, 0]);
        let mut invalid = bytes.clone();
        invalid[14..18].copy_from_slice(&(-32768i32).to_le_bytes());
        invalid[22..26].copy_from_slice(&32767i32.to_le_bytes());
        assert!(parse_animation(&invalid).is_none());
        assert!(parse_animation(&bytes[..bytes.len() - 1]).is_none());
    }
}
