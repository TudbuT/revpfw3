use enum_ordinalize::Ordinalize;

#[derive(Debug, PartialEq, Eq, Ordinalize)]
pub(crate) enum PacketType {
    NewClient,
    CloseClient,
    KeepAlive,
    ClientData,
    ServerData,
    ClientExceededBuffer,
}
