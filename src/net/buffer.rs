// -----------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system
//
// Copyright (c) 2022, Frank Sommer.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of the copyright holder nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
// -----------------------------------------------------------------------------------------------

//! Send and receive buffers for network communication.

use std::fmt::{Debug, Formatter};
use super::*;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;

/// Buffer to send Coaly messages across the network.
/// The buffer maintains the message header and payload parts as follows:
/// Bytes 0..3 - protocol information (byte 3 contains version)
/// Bytes 4..11 - application ID
/// Bytes 12..19 - message sequence number
/// Bytes 20..23 - payload size
/// Bytes 24.. - payload (byte 24 contains message ID)
pub struct SendBuffer {
    // protocol information, currently only version
    protocol_info: u32,
    // buffer for serialized messages
    buffer: Vec<u8>,
    // last sequence number used for a log/trace record
    sequence_nr: u64
}
impl SendBuffer {
    /// Creates a buffer to send Coaly messages across the network.
    /// 
    /// # Arguments
    /// * `protocol_info` - message protocol information, currently only last byte used to indicate
    ///                     protocol version
    /// * `app_id` - the application ID to use
    /// * `buffer_size` - the initial size of the internal byte buffer
    pub fn new(protocol_info: u32,
               buffer_size: usize) -> SendBuffer {
        let mut buffer = Vec::<u8>::with_capacity(buffer_size);
        protocol_info.serialize_to(&mut buffer);
        SendBuffer {
            protocol_info,
            buffer,
            sequence_nr: 0
        }
    }

    /// Returns the internal byte buffer as slice.
    /// Used as a parameter in the send calls to a network socket.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 { self.buffer.as_ptr() }

    /// Returns the internal byte buffer as slice.
    /// Used as a parameter in the send calls to a network socket.
    #[inline]
    pub fn as_slice(&self) -> &[u8] { self.buffer.as_slice() }

    /// Clears the internal buffer
    pub fn clear(&mut self) {
        self.sequence_nr = 0;
        self.buffer.truncate(4);
    }

    /// Stores a connect request message in the internal buffer.
    /// Connection ID is set to 0.
    /// 
    /// # Arguments
    /// * `orig_info` - information about the client needed by the trace server
    pub fn store_client_notification(&mut self, orig_info: &OriginatorInfo) {
        self.buffer.truncate(4);
        // sequence number
        0u64.serialize_to(&mut self.buffer);
        let payload_size = 1 + orig_info.serialized_size() as u32;
        payload_size.serialize_to(&mut self.buffer);
        self.buffer.push(CLIENT_NOTIF_ID);
        orig_info.serialize_to(&mut self.buffer);
    }

    /// Stores a RecordNotification message in the internal buffer.
    /// Used by the client to send a log or trace record to the server.
    /// 
    /// # Arguments
    /// * `record` - the log or trace record
    pub fn store_record_notification(&mut self, record: &dyn RecordData) {
        self.buffer.truncate(4);
        self.sequence_nr += 1;
        self.sequence_nr.serialize_to(&mut self.buffer);
        let payload_size = 1 + record.serialized_size() as u32;
        payload_size.serialize_to(&mut self.buffer);
        self.buffer.push(RECORD_NOTIF_ID);
        record.serialize_to(&mut self.buffer);
    }

    /// Stores a DisconnectNotification message in the internal buffer.
    /// Used by the client to indicate it will stop sending log or trace messages from now on.
    pub fn store_disconnect_notification(&mut self) {
        self.buffer.truncate(4);
        self.sequence_nr += 1;
        self.sequence_nr.serialize_to(&mut self.buffer);
        // payload size
        1u32.serialize_to(&mut self.buffer);
        self.buffer.push(DISCONNECT_NOTIF_ID);
    }

    /// Stores a shutdown request message in the internal buffer.
    /// Connection ID is set to 0.
    /// 
    /// # Arguments
    /// * `key` - the access key required by the server for administraive messages
    pub fn store_shutdown_request(&mut self, key: &str) {
        self.buffer.truncate(4);
        // sequence number
        0u64.serialize_to(&mut self.buffer);
        // payload size
        let payload_size = 1 + key.serialized_size() as u32;
        payload_size.serialize_to(&mut self.buffer);
        self.buffer.push(SHUTDOWN_REQ_ID);
        key.serialize_to(&mut self.buffer);
    }

    /// Stores an Shutdown response message in the internal buffer.
    /// Used by the server to indicate that a shutdown request is accepted.
    pub fn store_shutdown_response(&mut self) {
        self.buffer.truncate(4);
        // sequence number
        0u64.serialize_to(&mut self.buffer);
        // payload size
        1u32.serialize_to(&mut self.buffer);
        self.buffer.push(SHUTDOWN_RESP_ID);
    }

    /// Returns the payload size, 0 if buffer does not contain a payload.
    fn payload_size(&self) -> u32 {
        if self.buffer.len() < 16 { return 0u32 }
        u32::deserialize_from(&self.buffer[12..]).unwrap_or(0u32)
    }

    /// Returns the payload as hex string, "-" if buffer does not contain a payload.
    fn payload(&self) -> String {
        if self.buffer.len() <= 16 { return String::from("-") }
        let mut payload = String::with_capacity((self.buffer.len() - 16) << 1);
        self.buffer[16..].iter().for_each(|b| payload.push_str(&format!("{:02x}",b)));
        payload
    }
}
impl Debug for SendBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PROT:{}/SEQ:{}/LEN:{}/PSZ:{}/PLD:{}",
               self.protocol_info,
               self.sequence_nr,
               self.buffer.len(),
               self.payload_size(),
               self.payload())
    }
}

/// Buffer to receive Coaly messages from the network.
pub struct ReceiveBuffer {
    // protocol information, currently only version
    _protocol_info: u32,
    // buffer for serialized messages
    buffer: Vec<u8>
}
impl ReceiveBuffer {
    /// Creates a buffer to receive Coaly messages from the network.
    /// 
    /// # Arguments
    /// * `protocol_info` - message protocol information, currently only last byte used to indicate
    ///                     protocol version
    /// * `buffer_size` - the initial size of the internal byte buffer
    pub fn new(protocol_info: u32,
               buffer_size: usize) -> ReceiveBuffer {
        let actual_buf_size = usize::max(buffer_size, 128);
        let mut buffer = Vec::<u8>::with_capacity(actual_buf_size);
        unsafe { buffer.set_len(actual_buf_size); }
        ReceiveBuffer { _protocol_info: protocol_info, buffer }
    }

    /// Returns the internal byte buffer, needed as a parameter in the receive calls to a network
    /// socket.
    pub fn as_mut_ptr(&mut self) -> *mut u8 { self.buffer.as_mut_ptr() }

    /// Returns the internal byte buffer, needed as a parameter in the receive calls to a network
    /// socket.
    pub fn as_mut_slice(&mut self) -> &mut [u8] { self.buffer.as_mut_slice() }

    /// Returns the received protocol information, 0 if buffer contains less than 4 bytes
    pub fn protocol_info(&self) -> u32 {
        u32::deserialize_from(&self.buffer).unwrap_or(0u32)
    }

    /// Returns the received message sequence number, 0 if buffer contains less than 20 bytes
    pub fn sequence_nr(&self) -> u64 {
        u64::deserialize_from(&self.buffer[4..]).unwrap_or(0u64)
    }

    /// Returns the received payload size, 0 if buffer contains less than 28 bytes
    pub fn payload_size(&self) -> u32 {
        u32::deserialize_from(&self.buffer[12..]).unwrap_or(0u32)
    }

    /// Returns the received message from the internal buffer.
    /// Protocol information is currently ignored.
    /// 
    /// # Arguments
    /// * `bytes_received` - the number of bytes received from the socket
    pub fn message(&self, bytes_received: usize) -> Result<Message, CoalyException> {
        // minimum message size is 17 bytes
        if bytes_received < 17 { return Err(coalyxe!(E_MSG_TOO_SHORT)) }
        // check bytes received against received payload size
        if self.payload_size() as usize != bytes_received - 16 {
            return Err(coalyxe!(E_MSG_SIZE_MISMATCH, bytes_received.to_string(),
                                                   self.payload_size().to_string()))
        }
        Message::deserialize_from(&self.buffer[16..])
    }
}
impl Debug for ReceiveBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PROT:{}/SEQ:{}/PSZ:{}",
               self.protocol_info(),
               self.sequence_nr(),
               self.payload_size())
    }
}

#[cfg(all(net, test))]
mod tests {
    use super::*;
    use crate::record::RecordLevelId;
    use crate::record::recorddata::LocalRecordData;

    #[test]
    fn test_client_send() {
        let mut send_buf = SendBuffer::new(1, 256);
        let mut oinfo = OriginatorInfo::new(1234, "testapp", "clienthost", "1.2.3.4");
        oinfo.set_application_name("superapp");
        send_buf.store_client_notification(&oinfo);
        assert_eq!("PROT:1/SEQ:0/LEN:97/PSZ:85/PLD:0b",
                   &format!("{}", &send_buf)[..33]);
        let rec_txt = LocalRecordData::for_write(1234, "mythread", RecordLevelId::Error, 
                                                 "test.rs", 393, "blabla");
        send_buf.store_record_notification(&rec_txt);
        assert_eq!("PROT:1/SEQ:1/LEN:102/PSZ:90/PLD:0c",
                   &format!("{}", &send_buf)[..34]);
        send_buf.store_disconnect_notification();
        assert_eq!("PROT:1/SEQ:2/LEN:17/PSZ:1/PLD:0d", format!("{}", &send_buf));
    }

    #[test]
    fn test_shutdown() {
        let mut send_buf = SendBuffer::new(1, 256);
        send_buf.store_shutdown_request("TOPSECRET");
        assert_eq!("PROT:1/SEQ:0/LEN:34/PSZ:18/PLD:15",
                   &format!("{}", &send_buf)[..33]);
        send_buf.clear();
        assert_eq!("PROT:1/SEQ:0/LEN:4/PSZ:0/PLD:-", format!("{}", &send_buf));
        send_buf.store_shutdown_response();
        assert_eq!("PROT:1/SEQ:0/LEN:17/PSZ:1/PLD:1f", format!("{}", &send_buf));
    }

    fn check_recv(buf: &mut ReceiveBuffer,
                  hex_msg: &str,
                  expected_header: &str,
                  expected_msg: &Message) {
        fill_buf(buf, hex_msg);
        assert_eq!(expected_header, format!("{}", &buf));
        let hex_data_size = hex_msg.len() >> 1;
        let m = buf.message(hex_data_size);
        assert!(m.is_ok());
        assert_eq!(std::mem::discriminant(expected_msg), std::mem::discriminant(&m.unwrap()));
    }
    fn fill_buf(buf: &mut ReceiveBuffer, s: &str) {
        let hexdata = (0..s.len())
                       .step_by(2)
                       .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                       .collect::<Vec<u8>>();
        unsafe {
            let mut s_ptr = hexdata.as_ptr();
            let mut buf_ptr = buf.as_mut_ptr();
            for _ in 0..hexdata.len() {
                *buf_ptr = *s_ptr;
                s_ptr = s_ptr.add(1);
                buf_ptr = buf_ptr.add(1);
            }
        }
    }

}