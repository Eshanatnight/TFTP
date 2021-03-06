# TFTP

An app that uses Trivial File Transfer Protocol(TFTP) to send around files, specifically file streams/ data streams with UDP.

## Introduction

[Trivial File Transfer Protocol](https://en.wikipedia.org/wiki/Trivial_File_Transfer_Protocol) (TFTP) is a simple [lockstep](https://en.wikipedia.org/wiki/Lockstep_(computing)) [File Transfer Protocol](https://en.wikipedia.org/wiki/File_Transfer_Protocol) which allows a client to get a file from or put a file onto a remote host.

TFTP is designed to be small and easy to implement. Therefore, It's a nice protocol to studying about network.

Becouse of simplicity, TFTP use very small memory footprint. Ideal for [embedded systems](https://en.wikipedia.org/wiki/Embedded_system).

Today, TFTP is virtually unused for Internet transfers, Generally only used on [local area networks](https://en.wikipedia.org/wiki/Local_area_network) (LAN)

It implemented on top of the [UDP/IP](https://en.wikipedia.org/wiki/UDP/IP).

## Testing it Out

Clone the repository

```shell
git clone https://github.com/Eshanatnight/TFTP.git
```

```shell
cd ./TFTP
```

```shell
cargo r --examples basic
```

## TODO

- [ ] Add a FTP example (Server-Client Talk)
