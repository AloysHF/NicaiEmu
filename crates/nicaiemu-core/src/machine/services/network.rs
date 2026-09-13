//! Guest network manager services (group 9).
//!
//! Online CBE titles register a channel, send or request data, and wait for
//! asynchronous firmware callbacks. The host models that ABI with an in-memory
//! channel table and a deferred event queue; responses are produced by the
//! mock layer rather than a live socket.

use anyhow::Result;
use armv4t_emu::Memory;
use serde::{Deserialize, Serialize};

use super::super::NicaiMachine;

/// Maximum simultaneous guest network channels.
pub(crate) const MAX_NET_CHANNELS: usize = 8;
/// Maximum queued asynchronous network events.
const MAX_NET_EVENTS: usize = 32;
/// Maximum uplink bytes retained per send for mock inspection.
const MAX_CAPTURED_UPLINK: usize = 512;

/// Event types delivered to the guest network callback in `r3`.
pub(crate) const NET_EVENT_DATA: u32 = 0;
pub(crate) const NET_EVENT_COMPLETE: u32 = 1;
pub(crate) const NET_EVENT_CHANNEL_READY: u32 = 5;

/// A registered guest network channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct NetChannel {
    pub(crate) connect_id: u32,
    pub(crate) callback: u32,
    pub(crate) context: u32,
    pub(crate) active: bool,
}

/// A deferred network callback event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NetEvent {
    pub(crate) event_type: u32,
    pub(crate) r0: u32,
    pub(crate) r1: u32,
    pub(crate) r2: u32,
    pub(crate) callback: u32,
    pub(crate) context: u32,
    /// Frames remaining before the callback fires. Zero means due now.
    pub(crate) delay_frames: u32,
}

impl NicaiMachine {
    pub(crate) fn handle_network_service(&mut self, index: u32) {
        match index {
            0 => self.network_connect(),
            1 => self.network_send(),
            2 => self.network_close(),
            3 => self.network_http_get(),
            // Connectivity / configuration probes the guest treats as success.
            4 | 19 | 20 | 29 | 30 => self.set_result(1),
            35 => {
                self.net_uplink_bytes = 0;
                self.net_downlink_bytes = 0;
                self.net_last_uplink.clear();
                self.set_result(0);
            }
            36 => {
                let uplink_ptr = self.register(0);
                let downlink_ptr = self.register(1);
                if uplink_ptr != 0 {
                    self.memory.w32(uplink_ptr, self.net_uplink_bytes as u32);
                }
                if downlink_ptr != 0 {
                    self.memory
                        .w32(downlink_ptr, self.net_downlink_bytes as u32);
                }
                self.set_result(self.net_downlink_bytes as u32);
            }
            _ => self.set_result(0),
        }
    }

    /// Register an asynchronous network channel (`connect`).
    ///
    /// ABI: r0/r1 = request cookies, r2 = guest callback, r3 = out-pointer for
    /// the generated connect id (optional). Returns nonzero on success.
    fn network_connect(&mut self) {
        let callback = self.register(2);
        let connect_id_out = self.register(3);
        if callback == 0 {
            self.set_result(0);
            return;
        }
        let Some(connect_id) = self.allocate_net_channel(callback, 0) else {
            self.set_result(0);
            return;
        };
        if connect_id_out != 0 {
            self.memory.w32(connect_id_out, connect_id);
        }
        self.queue_net_event(NET_EVENT_CHANNEL_READY, 0, 0, 0, callback, 0, 1);
        self.set_result(1);
    }

    /// Accept an uplink packet and queue a mock completion.
    ///
    /// ABI: r0 = payload pointer, r1 = payload length, r2 = connect id.
    fn network_send(&mut self) {
        let data_ptr = self.register(0);
        let data_len = self.register(1);
        let connect_id = self.register(2);
        if data_ptr == 0 || data_len == 0 {
            self.set_result(0);
            return;
        }
        let read_len = data_len.min(MAX_CAPTURED_UPLINK as u32) as usize;
        let mut captured = vec![0u8; read_len];
        for (offset, byte) in captured.iter_mut().enumerate() {
            *byte = self.memory.r8(data_ptr + offset as u32);
        }
        self.net_last_uplink = captured;
        self.net_uplink_bytes = self.net_uplink_bytes.saturating_add(data_len as u64);
        if let Some((callback, context)) = self.channel_callback(connect_id) {
            let response = self.build_mock_uplink_response();
            self.queue_mock_response(callback, context, &response);
        }
        self.set_result(data_len);
    }

    /// Unregister a network channel.
    ///
    /// ABI: r0 = connect id.
    fn network_close(&mut self) {
        let connect_id = self.register(0);
        for channel in &mut self.net_channels {
            if channel.active && channel.connect_id == connect_id {
                channel.active = false;
            }
        }
        self.set_result(0);
    }

    /// Issue an HTTP-style GET and queue the mock body.
    ///
    /// ABI: r0 = URL pointer, r1 = guest callback, r2 = context.
    fn network_http_get(&mut self) {
        let url_ptr = self.register(0);
        let callback = self.register(1);
        let context = self.register(2);
        if callback == 0 {
            self.set_result(0);
            return;
        }
        let url = self.read_guest_cstring(url_ptr, 256);
        self.net_last_http_url = url.clone();
        let response = self.build_mock_http_response(&url);
        self.queue_mock_response(callback, context, &response);
        self.set_result(1);
    }

    fn allocate_net_channel(&mut self, callback: u32, context: u32) -> Option<u32> {
        let slot = self
            .net_channels
            .iter()
            .position(|channel| !channel.active)?;
        self.next_net_connect_id = self.next_net_connect_id.wrapping_add(1).max(1);
        let connect_id = self.next_net_connect_id;
        self.net_channels[slot] = NetChannel {
            connect_id,
            callback,
            context,
            active: true,
        };
        Some(connect_id)
    }

    fn channel_callback(&self, connect_id: u32) -> Option<(u32, u32)> {
        self.net_channels
            .iter()
            .find(|channel| channel.active && channel.connect_id == connect_id)
            .map(|channel| (channel.callback, channel.context))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn queue_net_event(
        &mut self,
        event_type: u32,
        r0: u32,
        r1: u32,
        r2: u32,
        callback: u32,
        context: u32,
        delay_frames: u32,
    ) {
        if self.net_events.len() >= MAX_NET_EVENTS || callback == 0 {
            return;
        }
        self.net_events.push_back(NetEvent {
            event_type,
            r0,
            r1,
            r2,
            callback,
            context,
            delay_frames,
        });
    }

    /// Copy `bytes` into guest heap and queue a data event followed by complete.
    pub(crate) fn queue_mock_response(&mut self, callback: u32, context: u32, bytes: &[u8]) {
        let payload_len = bytes.len() as u32;
        let payload_ptr = if payload_len == 0 {
            0
        } else {
            let ptr = self.allocate(payload_len);
            if ptr == 0 {
                return;
            }
            if !self.memory.write_bytes(ptr, bytes) {
                return;
            }
            ptr
        };
        self.net_downlink_bytes = self.net_downlink_bytes.saturating_add(payload_len as u64);
        // Deliver data on the next guest tick, completion one tick later so
        // firmware that processes the body first still observes a trailing
        // complete event.
        self.queue_net_event(
            NET_EVENT_DATA,
            payload_ptr,
            payload_len,
            payload_len,
            callback,
            context,
            1,
        );
        self.queue_net_event(NET_EVENT_COMPLETE, 0, 0, 0, callback, context, 2);
    }

    /// Advance queued network events and invoke due guest callbacks.
    pub(crate) fn dispatch_network_events(&mut self, instruction_limit: u64) -> Result<()> {
        let mut due = Vec::new();
        let mut remaining = std::collections::VecDeque::new();
        for event in self.net_events.drain(..) {
            if event.delay_frames > 0 {
                remaining.push_back(NetEvent {
                    delay_frames: event.delay_frames - 1,
                    ..event
                });
            } else {
                due.push(event);
            }
        }
        self.net_events = remaining;
        for event in due {
            self.invoke_network_callback(&event, instruction_limit)?;
        }
        Ok(())
    }

    fn invoke_network_callback(&mut self, event: &NetEvent, instruction_limit: u64) -> Result<()> {
        if event.callback == 0 {
            return Ok(());
        }
        // Firmware callback ABI: r0/r1/r2 carry the payload view and r3 is
        // the event type. Context stays associated with the channel table.
        self.invoke_callback_with_r3(
            event.callback,
            event.r0,
            event.r1,
            event.r2,
            event.event_type,
            instruction_limit,
        )
    }

    fn read_guest_cstring(&mut self, pointer: u32, max_len: usize) -> String {
        if pointer == 0 {
            return String::new();
        }
        let mut bytes = Vec::new();
        for offset in 0..max_len as u32 {
            let byte = self.memory.r8(pointer + offset);
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Default mock body: a single success byte is enough for status probes.
    fn build_mock_http_response(&self, url: &str) -> Vec<u8> {
        let _ = url;
        vec![1]
    }

    /// Default mock body for raw uplink packets.
    fn build_mock_uplink_response(&self) -> Vec<u8> {
        vec![1]
    }
}
