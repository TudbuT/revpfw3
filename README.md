 Reverse-PortForward V3
========================

This tool bypasses port restrictions of your router using some not-very-powerful
server (a cheap 1€ vserver will suffice.)

NEW: Modem support! RevPFW3 can now interact with modems using AT commands. A demo
is included for the SIM800L GSM modem.

---

### How to download it

I will provide a windows and mac build shortly. Right now, I can only provide a linux
build.

All builds I make can be found in the
[releases](https://github.com/tudbut/revpfw3/releases/latest).

If my build doesn't work or your system doesn't have one, [install
Rust](https://rustup.rs) and run `cargo install revpfw3` or `cargo install --git
https://github.com/tudbut/revpfw3`.

---

### How to set it up:

1. Buy some cheap server online, it will only need
   1. Enough disk space to run a 5MB program (I recommend about .5GB free after
      OS is installed)
   2. 50MB of free RAM or more (if you expect to have many clients connecting, 
      use more RAM)
   3. Flexible port settings
   4. Not much CPU power, a single core definitely suffices.
2. Download revpfw3 to it
3. Run it like this: `revpfw3 server <port> <key>` (I recommend doing it in a
   loop)
4. Download it to your destination as well (your PC, a raspi, etc)
5. Run it like this: `revpfw3 client <ip of your bridge server> <port> localhost
   <port to redirect (on local machine)> <key>`
6. To restart, end BOTH processes (remote and on your local server) and restart
   them.

---

### Applications and special features:

- Minecraft servers tested and functional.
- HTTP tested and functional.
- Some third-party protocols tested and functional.
- This is not an HTTP-Proxy. It will work with any TCP protocol that isn't
  reliant on TCPNODELAY.
- No disconnects, even when the sockets stay open for hours.
- Fast
- Little ping increase in normal applications
- A 1ms waiting delay before sending is built in to reduce stress and increase
  efficiency by waiting for further data.

---

### As a rust library

Reverse-PortForward V3 supports being used as a library. `revpfw3::client` and
`revpfw3::server` are public, so you can use those. Keep in mind they will panic
when the connection to the corresponding client/server drops.

