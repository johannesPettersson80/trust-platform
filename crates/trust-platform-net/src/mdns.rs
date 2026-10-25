//! The chip's Multicast DNS responder (specification 34, ARCH-12 and the checklist's S6;
//! specification 11 §6.12). A chip on the network is found by the same browse as every
//! trust-runtime, a DNS-SD browse of `_trust._plc._tcp.local.` (RFC 6763 over Multicast DNS, RFC
//! 6762), so it answers with the records that a computer's announcement gives: the PTR record that
//! names its instance, the instance's SRV record with its host and web port, its TXT record with
//! the properties `id`, `name`, `version`, `platform`, `auth` and the optional ones, and its host's
//! A record.
//!
//! [`answer`] answers one query that arrived on UDP port [`MDNS_PORT`]: the PTR question of the
//! service type, with the SRV, TXT and A records as additional records (RFC 6763 §12.1); the PTR
//! question of the service type enumeration (§9); the SRV, TXT and ANY questions of the instance;
//! and the A and ANY questions of the host. [`announcement`] writes the unsolicited response that
//! announces the records (RFC 6762 §8.3) or withdraws them (§10.1).
//!
//! Nothing here sends, receives or reads a clock; the chip's network loop does, by these rules of
//! RFC 6762. It answers only queries whose source address is on the local link (§11). It sends
//! every reply from UDP port 5353 (§6) with an IP time to live of 255 (§11): a
//! [`Reply::Multicast`] to [`MDNS_GROUP`] port [`MDNS_PORT`], and a [`Reply::Unicast`] to the
//! query's source address and port. It multicasts a record no more often than once a second (§6),
//! and may delay a multicast reply to a browse by 20 to 120 ms, the PTR record being shared with
//! the other instances of the service (§6). It sends [`announcement`] with [`Announce::Hello`] at
//! least twice, one second apart, when the link comes up and whenever a record changes (§8.3,
//! §8.4), and with [`Announce::Goodbye`] before the chip leaves the network (§10.1).
//!
//! Names are written without compression, which RFC 1035 §4.1.4 allows; a query's names are read
//! with it. Not implemented: probing and conflict resolution (RFC 6762 §8.1, §8.2, §9), since the
//! names come from the chip's settings, whose device name is checked; known-answer suppression
//! (§7.1), duplicate question and answer suppression (§7.3, §7.4) and the choice of a multicast
//! reply to a unicast-response question whose records were not multicast lately (§5.4), which
//! need a clock and only save traffic; and negative responses (NSEC, §6.1) and AAAA records, the
//! chip having an IPv4 address only.

/// The DNS-SD service type that every truST runtime announces and browses (specification 11
/// §6.12), written without the root's final dot.
pub const SERVICE: &str = "_trust._plc._tcp.local";
/// The UDP port of Multicast DNS (RFC 6762 §3).
pub const MDNS_PORT: u16 = 5353;
/// The IPv4 group of Multicast DNS, 224.0.0.251 (RFC 6762 §3).
pub const MDNS_GROUP: [u8; 4] = [224, 0, 0, 251];

/// The name of DNS-SD's service type enumeration (RFC 6763 §9).
const SERVICES: &str = "_services._dns-sd._udp.local";
/// The domain of a Multicast DNS host name (RFC 6762 §3).
const LOCAL: &str = "local";

/// The bytes of a DNS message's header (RFC 1035 §4.1.1).
const HEADER_BYTES: usize = 12;
/// The longest label (RFC 1035 §2.3.4).
const MAX_LABEL_BYTES: usize = 63;
/// The longest name, with its length bytes and its root (RFC 1035 §2.3.4).
const MAX_NAME_BYTES: usize = 255;
/// The longest string of a TXT record, after its length byte (RFC 6763 §6.1).
const MAX_STRING_BYTES: usize = 255;
/// The most compression pointers that one name of a query may follow.
const MAX_POINTER_HOPS: usize = 16;

/// The header flag of a response, QR (RFC 1035 §4.1.1; RFC 6762 §18.2).
const FLAG_RESPONSE: u16 = 0x8000;
/// The header bits of the OPCODE, 0 in every Multicast DNS message (RFC 6762 §18.3).
const FLAG_OPCODE: u16 = 0x7800;
/// The header flag of an authoritative answer, AA, which every Multicast DNS response sets (RFC
/// 6762 §18.4).
const FLAG_AUTHORITATIVE: u16 = 0x0400;
/// The header flag of recursion desired, RD, which a conventional reply copies from its query (RFC
/// 1035 §4.1.1).
const FLAG_RECURSION_DESIRED: u16 = 0x0100;
/// The header bits of the RCODE, 0 in every Multicast DNS message (RFC 6762 §18.11).
const FLAG_RCODE: u16 = 0x000F;

/// The type of an IPv4 address record (RFC 1035 §3.2.2).
const TYPE_A: u16 = 1;
/// The type of a pointer record (RFC 1035 §3.2.2).
const TYPE_PTR: u16 = 12;
/// The type of a text record (RFC 1035 §3.2.2).
const TYPE_TXT: u16 = 16;
/// The type of a service record (RFC 2782).
const TYPE_SRV: u16 = 33;
/// The question type that asks for every type (RFC 1035 §3.2.3).
const TYPE_ANY: u16 = 255;
/// The Internet class (RFC 1035 §3.2.4).
const CLASS_IN: u16 = 1;
/// The question class that asks for every class (RFC 1035 §3.2.5; RFC 6762 §6.5).
const CLASS_ANY: u16 = 255;
/// The top bit of a question's class: the querier asks for a unicast reply (RFC 6762 §5.4, §18.12).
const UNICAST_RESPONSE: u16 = 0x8000;
/// The top bit of a record's class in a response: the cache-flush bit of a unique record (RFC 6762
/// §10.2, §18.13).
const CACHE_FLUSH: u16 = 0x8000;

/// The time to live of a record that names a host, the SRV and the A record, in seconds (RFC 6762
/// §10).
const HOST_TTL: u32 = 120;
/// The time to live of the other records, 75 minutes, in seconds (RFC 6762 §10).
const OTHER_TTL: u32 = 4_500;
/// The longest time to live of a legacy unicast reply, in seconds (RFC 6762 §6.7).
const LEGACY_TTL: u32 = 10;

/// What a chip announces: the same service as a computer's trust-runtime (specification 11 §6.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Announcement<'a> {
    /// The instance label, `<service_name>-<runtime_name>` as a computer names it, such as
    /// `truST-line-3`: one label of 1 to 63 bytes, without the service type.
    pub instance: &'a str,
    /// The host label, such as `line-3`: one label of 1 to 63 bytes, without `.local`.
    pub host: &'a str,
    /// The chip's IPv4 address, which the A record of `<host>.local` gives.
    pub address: [u8; 4],
    /// The port of the SRV record: the web port, as a computer's announcement gives it.
    pub port: u16,
    /// The TXT properties in their order: `id`, `name`, `version`, `platform`, `auth` and the
    /// optional `web_port`, `web_tls`, `mesh_port`, `control` and `host_group` (specification 11
    /// §6.12), each written as `key=value` in at most 255 bytes.
    pub txt: &'a [(&'a str, &'a str)],
}

/// A reply that [`answer`] wrote into the caller's buffer: its length in bytes, and where the
/// network loop sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    /// To the group, [`MDNS_GROUP`] port [`MDNS_PORT`].
    Multicast(usize),
    /// To the query's source address and port: every question answered asked for a unicast reply
    /// (RFC 6762 §5.4), or the query came from a port other than 5353, a legacy query (§6.7).
    Unicast(usize),
}

/// Which unsolicited response [`announcement`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announce {
    /// The records with their times to live, which announce the chip (RFC 6762 §8.3).
    Hello,
    /// The records with a time to live of 0, which make every browser drop the chip at once (RFC
    /// 6762 §10.1).
    Goodbye,
}

/// Answers `query`, a DNS message that arrived on port [`MDNS_PORT`] from `source_port`, about
/// `announcement`: writes the reply into `buf` and says where it goes, or gives `None` when there
/// is nothing to send.
///
/// A response, a message whose OPCODE or RCODE is not 0 (RFC 6762 §18.2, §18.3, §18.11), a
/// malformed message and a query that asks for none of the announcement's records get `None`; a
/// query's answer, authority and additional records are not read. A question of class IN or ANY,
/// its unicast-response bit aside, asks for the records of its name and its type, or of its name
/// and every type for type ANY: the PTR records of [`SERVICE`] and of
/// `_services._dns-sd._udp.local`, the SRV and TXT records of `<instance>._trust._plc._tcp.local`,
/// and the A record of `<host>.local`, the names compared without regard to ASCII case (RFC 1035
/// §2.3.3). An answer's PTR record of the instance brings the SRV, TXT and A records as additional
/// records, and its SRV record the A record (RFC 6763 §12.1, §12.2).
///
/// A query from port 5353 is answered to the group with ID 0, unless every question answered set
/// the unicast-response bit: then it is answered to the asker with the query's ID (RFC 6762 §5.4,
/// §18.1). A query from another port is a legacy query (§6.7), answered to the asker with the
/// query's ID, its questions repeated, its recursion-desired flag copied, every time to live at
/// most 10 s and no cache-flush bit.
///
/// Additional records that do not fit `buf` are left out. The reply is `None` when its answers do
/// not fit, and when the announcement's instance or host is not a label of 1 to 63 bytes or one of
/// its properties is longer than 255 bytes. `buf` may have been written to when the reply is
/// `None`.
#[must_use]
pub fn answer(
    query: &[u8],
    source_port: u16,
    announcement: &Announcement<'_>,
    buf: &mut [u8],
) -> Option<Reply> {
    if query.len() < HEADER_BYTES || !carried(announcement) {
        return None;
    }
    let id = read_word(query, 0)?;
    let flags = read_word(query, 2)?;
    let questions = read_word(query, 4)?;
    if (flags & (FLAG_RESPONSE | FLAG_OPCODE | FLAG_RCODE)) != 0 {
        return None;
    }
    let legacy = source_port != MDNS_PORT;
    let mut writer = Writer {
        buf,
        len: HEADER_BYTES,
    };
    let mut asked = Records::default();
    let mut multicast = false;
    let mut name = [0_u8; MAX_NAME_BYTES];
    let mut position = HEADER_BYTES;
    for _ in 0..questions {
        let (length, after) = read_name(query, position, &mut name)?;
        let wire = name.get(..length)?;
        let question_type = read_word(query, after)?;
        let question_class = read_word(query, after.checked_add(2)?)?;
        position = after.checked_add(4)?;
        if legacy {
            writer.bytes(wire)?;
            writer.put16(question_type)?;
            writer.put16(question_class)?;
        }
        let records = matching(wire, question_type, question_class, announcement);
        if !records.is_empty() {
            asked = asked.union(records);
            multicast |= (question_class & UNICAST_RESPONSE) == 0;
        }
    }
    if asked.is_empty() {
        return None;
    }
    let ttl = if legacy { Ttl::Legacy } else { Ttl::Own };
    let mut answers: u16 = 0;
    for record in RECORDS {
        if asked.contains(record) {
            write_record(&mut writer, record, announcement, ttl)?;
            answers += 1;
        }
    }
    let extra = additional(asked);
    let mut additionals: u16 = 0;
    for record in RECORDS {
        if extra.contains(record) {
            let before = writer.len;
            if write_record(&mut writer, record, announcement, ttl).is_some() {
                additionals += 1;
            } else {
                writer.len = before;
            }
        }
    }
    let unicast = legacy || !multicast;
    let recursion = if legacy {
        flags & FLAG_RECURSION_DESIRED
    } else {
        0
    };
    writer.header([
        if unicast { id } else { 0 },
        FLAG_RESPONSE | FLAG_AUTHORITATIVE | recursion,
        if legacy { questions } else { 0 },
        answers,
        0,
        additionals,
    ])?;
    Some(if unicast {
        Reply::Unicast(writer.len)
    } else {
        Reply::Multicast(writer.len)
    })
}

/// Writes into `buf` the unsolicited response of `kind` about `announcement` and gives its length:
/// ID 0, the flags QR and AA, and the PTR record of the instance, its SRV and TXT records and the
/// host's A record as answers (RFC 6762 §8.3), with their times to live for [`Announce::Hello`]
/// and 0 for [`Announce::Goodbye`] (§10.1), the cache-flush bit on the SRV, TXT and A records
/// (§10.2). `None` when it does not fit `buf`, or when the announcement's instance or host is not a
/// label of 1 to 63 bytes or one of its properties is longer than 255 bytes.
#[must_use]
pub fn announcement(
    announcement: &Announcement<'_>,
    kind: Announce,
    buf: &mut [u8],
) -> Option<usize> {
    if !carried(announcement) {
        return None;
    }
    let ttl = match kind {
        Announce::Hello => Ttl::Own,
        Announce::Goodbye => Ttl::Zero,
    };
    let mut writer = Writer {
        buf,
        len: HEADER_BYTES,
    };
    for record in ANNOUNCED {
        write_record(&mut writer, record, announcement, ttl)?;
    }
    let answers = u16::try_from(ANNOUNCED.len()).ok()?;
    writer.header([0, FLAG_RESPONSE | FLAG_AUTHORITATIVE, 0, answers, 0, 0])?;
    Some(writer.len)
}

/// The records of an announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Record {
    /// [`SERVICE`] PTR `<instance>._trust._plc._tcp.local`, which a browse asks for (RFC 6763 §4).
    InstancePtr,
    /// `_services._dns-sd._udp.local` PTR [`SERVICE`], the service type enumeration (RFC 6763 §9).
    TypePtr,
    /// The instance's SRV record: priority 0, weight 0, the port and `<host>.local` (RFC 6763 §5).
    Srv,
    /// The instance's TXT record: the properties (RFC 6763 §6).
    Txt,
    /// The A record of `<host>.local`: the chip's address.
    A,
}

/// Every record, in the order a reply writes them.
const RECORDS: [Record; 5] = [
    Record::InstancePtr,
    Record::TypePtr,
    Record::Srv,
    Record::Txt,
    Record::A,
];

/// The records an announcement sends, in its order (RFC 6762 §8.3).
const ANNOUNCED: [Record; 4] = [Record::InstancePtr, Record::Srv, Record::Txt, Record::A];

impl Record {
    /// The record's bit in a [`Records`] set.
    fn bit(self) -> u8 {
        match self {
            Self::InstancePtr => 0x01,
            Self::TypePtr => 0x02,
            Self::Srv => 0x04,
            Self::Txt => 0x08,
            Self::A => 0x10,
        }
    }

    /// The record's type.
    fn rtype(self) -> u16 {
        match self {
            Self::InstancePtr | Self::TypePtr => TYPE_PTR,
            Self::Srv => TYPE_SRV,
            Self::Txt => TYPE_TXT,
            Self::A => TYPE_A,
        }
    }

    /// Whether the record is the chip's alone, so that a reply sets its cache-flush bit (RFC 6762
    /// §10.2): the SRV, TXT and A records. The PTR records are shared with the other instances of
    /// the service and the other service types.
    fn unique(self) -> bool {
        matches!(self, Self::Srv | Self::Txt | Self::A)
    }

    /// The record's own time to live (RFC 6762 §10): 120 s for the records that name the host, the
    /// SRV and the A record, and 75 minutes for the others.
    fn own_ttl(self) -> u32 {
        match self {
            Self::Srv | Self::A => HOST_TTL,
            Self::InstancePtr | Self::TypePtr | Self::Txt => OTHER_TTL,
        }
    }

    /// The record's name: its first label, when the announcement gives it, and the dot-separated
    /// labels after it.
    fn owner<'a>(self, announcement: &Announcement<'a>) -> (Option<&'a str>, &'static str) {
        match self {
            Self::InstancePtr => (None, SERVICE),
            Self::TypePtr => (None, SERVICES),
            Self::Srv | Self::Txt => (Some(announcement.instance), SERVICE),
            Self::A => (Some(announcement.host), LOCAL),
        }
    }
}

/// A set of [`Record`]s.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Records(u8);

impl Records {
    /// The set with `record` added.
    #[must_use]
    fn with(self, record: Record) -> Self {
        Self(self.0 | record.bit())
    }

    /// The records of this set and of `other`.
    #[must_use]
    fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The records of this set that are not in `other`.
    #[must_use]
    fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Whether the set holds `record`.
    fn contains(self, record: Record) -> bool {
        (self.0 & record.bit()) != 0
    }

    /// Whether the set holds no record.
    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The times to live and cache-flush bits that a reply writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ttl {
    /// Each record's own time to live (RFC 6762 §10), the cache-flush bit on the unique records
    /// (§10.2).
    Own,
    /// A legacy unicast reply's: at most 10 s, and no cache-flush bit (RFC 6762 §6.7).
    Legacy,
    /// A goodbye's: 0 (RFC 6762 §10.1), the cache-flush bit on the unique records.
    Zero,
}

/// Whether DNS can carry `announcement`: its instance and its host are labels of 1 to 63 bytes
/// (RFC 1035 §2.3.4), and each property is a string of at most 255 bytes as `key=value` (RFC 6763
/// §6.1).
fn carried(announcement: &Announcement<'_>) -> bool {
    let label = |text: &str| (1..=MAX_LABEL_BYTES).contains(&text.len());
    label(announcement.instance)
        && label(announcement.host)
        && announcement.txt.iter().all(|(key, value)| {
            key.len().saturating_add(1).saturating_add(value.len()) <= MAX_STRING_BYTES
        })
}

/// The records that a question for `name` (uncompressed, as [`read_name`] gives it) of
/// `question_type` and `question_class` asks for: those of its name, of its type or of every type,
/// in class IN or every class (RFC 1035 §3.2.3, §3.2.5; RFC 6762 §6.5).
fn matching(
    name: &[u8],
    question_type: u16,
    question_class: u16,
    announcement: &Announcement<'_>,
) -> Records {
    let mut records = Records::default();
    let class = question_class & !UNICAST_RESPONSE;
    if class != CLASS_IN && class != CLASS_ANY {
        return records;
    }
    for record in RECORDS {
        let (first, rest) = record.owner(announcement);
        let of_type = question_type == record.rtype() || question_type == TYPE_ANY;
        if of_type && is_name(name, first, rest) {
            records = records.with(record);
        }
    }
    records
}

/// The additional records of a reply whose answers are `asked`, less those answers (RFC 6763
/// §12): the SRV, TXT and A records of the instance's PTR record, and the A record of an SRV
/// record.
fn additional(asked: Records) -> Records {
    let mut extra = Records::default();
    if asked.contains(Record::InstancePtr) {
        extra = extra.with(Record::Srv).with(Record::Txt).with(Record::A);
    }
    if asked.contains(Record::Srv) {
        extra = extra.with(Record::A);
    }
    extra.without(asked)
}

/// Whether `wire`, an uncompressed name as [`read_name`] gives it, is `first`, when given, and the
/// dot-separated labels of `rest`, without regard to ASCII case (RFC 1035 §2.3.3).
fn is_name(wire: &[u8], first: Option<&str>, rest: &str) -> bool {
    let mut remaining = wire;
    for label in first.into_iter().chain(rest.split('.')) {
        let Some((&length, tail)) = remaining.split_first() else {
            return false;
        };
        let Some((bytes, after)) = tail.split_at_checked(usize::from(length)) else {
            return false;
        };
        if label.is_empty() || !bytes.eq_ignore_ascii_case(label.as_bytes()) {
            return false;
        }
        remaining = after;
    }
    matches!(remaining, [0])
}

/// Reads the name at `start` of `message` into `name` without compression (RFC 1035 §3.1, §4.1.4)
/// and gives the name's length there and the position after the name in `message`: after its
/// first pointer, or after its root label.
///
/// `None` for a name that runs past the end of `message`, a label of a reserved type or longer
/// than 63 bytes, a name longer than 255 bytes, a pointer that points into the header or not back
/// before the labels it ends, and a name that follows more than 16 pointers. Since each pointer
/// points before the labels it ends, the positions a name is read from only fall, and a name
/// cannot loop.
fn read_name(
    message: &[u8],
    start: usize,
    name: &mut [u8; MAX_NAME_BYTES],
) -> Option<(usize, usize)> {
    let mut position = start;
    let mut earliest = start;
    let mut after = None;
    let mut hops = 0;
    let mut length = 0;
    loop {
        let byte = *message.get(position)?;
        match byte & 0xC0 {
            0x00 => {
                let end = position.checked_add(usize::from(byte))?.checked_add(1)?;
                let label = message.get(position..end)?;
                let written = length + label.len();
                name.get_mut(length..written)?.copy_from_slice(label);
                length = written;
                if byte == 0 {
                    return Some((length, after.unwrap_or(end)));
                }
                position = end;
            }
            0xC0 => {
                let low = *message.get(position.checked_add(1)?)?;
                let target = usize::from(u16::from_be_bytes([byte & 0x3F, low]));
                hops += 1;
                if hops > MAX_POINTER_HOPS || !(HEADER_BYTES..earliest).contains(&target) {
                    return None;
                }
                if after.is_none() {
                    after = Some(position.checked_add(2)?);
                }
                position = target;
                earliest = target;
            }
            _ => return None,
        }
    }
}

/// The 16-bit word at `at` of `message`, high byte first (RFC 1035 §2.3.2); `None` past its end.
fn read_word(message: &[u8], at: usize) -> Option<u16> {
    message
        .get(at..)?
        .first_chunk::<2>()
        .map(|bytes| u16::from_be_bytes(*bytes))
}

/// Appends `record` of `announcement`, with the time to live and the cache-flush bit that `ttl`
/// gives (RFC 1035 §4.1.3); `None` when it does not fit, or a label or a property cannot be
/// written.
fn write_record(
    writer: &mut Writer<'_>,
    record: Record,
    announcement: &Announcement<'_>,
    ttl: Ttl,
) -> Option<()> {
    let (first, rest) = record.owner(announcement);
    writer.name(first, rest)?;
    writer.put16(record.rtype())?;
    let flush = record.unique() && ttl != Ttl::Legacy;
    let class = if flush {
        CLASS_IN | CACHE_FLUSH
    } else {
        CLASS_IN
    };
    writer.put16(class)?;
    writer.put32(match ttl {
        Ttl::Own => record.own_ttl(),
        Ttl::Legacy => record.own_ttl().min(LEGACY_TTL),
        Ttl::Zero => 0,
    })?;
    let length_at = writer.len;
    writer.put16(0)?;
    match record {
        Record::InstancePtr => writer.name(Some(announcement.instance), SERVICE)?,
        Record::TypePtr => writer.name(None, SERVICE)?,
        Record::Srv => {
            // Priority 0 and weight 0: the one target (RFC 2782).
            writer.put16(0)?;
            writer.put16(0)?;
            writer.put16(announcement.port)?;
            writer.name(Some(announcement.host), LOCAL)?;
        }
        Record::Txt => write_txt(writer, announcement.txt)?,
        Record::A => writer.bytes(&announcement.address)?,
    }
    let data = writer.len.checked_sub(length_at)?.checked_sub(2)?;
    writer.set16(length_at, u16::try_from(data).ok()?)
}

/// Appends the data of a TXT record: each property as one string, `key=value` after its length
/// byte, and a single empty string, one zero byte, for no property (RFC 6763 §6.1). `None` for a
/// property longer than 255 bytes.
fn write_txt(writer: &mut Writer<'_>, txt: &[(&str, &str)]) -> Option<()> {
    if txt.is_empty() {
        return writer.bytes(&[0]);
    }
    for (key, value) in txt {
        let length = key.len().checked_add(1)?.checked_add(value.len())?;
        writer.bytes(&[u8::try_from(length).ok()?])?;
        writer.bytes(key.as_bytes())?;
        writer.bytes(b"=")?;
        writer.bytes(value.as_bytes())?;
    }
    Some(())
}

/// A message written into the caller's buffer, which refuses what does not fit.
struct Writer<'b> {
    /// The caller's buffer.
    buf: &'b mut [u8],
    /// The bytes written, the header's included.
    len: usize,
}

impl Writer<'_> {
    /// Appends `bytes`; `None`, with nothing appended, when they do not fit.
    fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.len.checked_add(bytes.len())?;
        self.buf.get_mut(self.len..end)?.copy_from_slice(bytes);
        self.len = end;
        Some(())
    }

    /// Appends a 16-bit word, high byte first (RFC 1035 §2.3.2).
    fn put16(&mut self, value: u16) -> Option<()> {
        self.bytes(&value.to_be_bytes())
    }

    /// Appends a 32-bit word, high byte first (RFC 1035 §2.3.2).
    fn put32(&mut self, value: u32) -> Option<()> {
        self.bytes(&value.to_be_bytes())
    }

    /// Writes `value` over the 16-bit word at `at`.
    fn set16(&mut self, at: usize, value: u16) -> Option<()> {
        let end = at.checked_add(2)?;
        self.buf
            .get_mut(at..end)?
            .copy_from_slice(&value.to_be_bytes());
        Some(())
    }

    /// Appends a name: `first`, when given, and the dot-separated labels of `rest`, each after its
    /// length byte, then the root's zero (RFC 1035 §3.1). `None` for a label that is empty or
    /// longer than 63 bytes.
    fn name(&mut self, first: Option<&str>, rest: &str) -> Option<()> {
        for label in first.into_iter().chain(rest.split('.')) {
            if label.is_empty() || label.len() > MAX_LABEL_BYTES {
                return None;
            }
            self.bytes(&[u8::try_from(label.len()).ok()?])?;
            self.bytes(label.as_bytes())?;
        }
        self.bytes(&[0])
    }

    /// Writes the header's six words: the ID, the flags and the counts of the questions, answers,
    /// authority records and additional records (RFC 1035 §4.1.1).
    fn header(&mut self, words: [u16; 6]) -> Option<()> {
        for (index, value) in words.into_iter().enumerate() {
            self.set16(index.checked_mul(2)?, value)?;
        }
        Some(())
    }
}

#[cfg(test)]
mod tests;
