# AnvilRegion Repacker

Useful utility to repacking & compacting Anvil Region files with high speed

It's fast! It's blazingly fast! Because it's Rust 🚀!  
(It's just a joke 😁)

# Quicky FAQ

## Does it work?

Currently, no. WIP.

## What exactly it does?

Minecraft holds chunks in files compressed with not-so-good algorithms. One compressed chunk usually weight around 500 bytes.  
But one sector of region file is 4096! Chunks are stored by sectors not by chunks. So, for small chunks there are overhead around ~75%.
Some sectors are unused due to chunk growing.

Does it mean Minecraft is bad? Absolutely not! It's optimized for random access to prevent full file rewrite for each time any chunk is updated.
But archives doesn't updating chunks. So here we are...

## Is this tested?

Nope =\)

## Is it stable now?

Nope. Cli usage can change 5 times at 1 second.

## Does packed file guaranted to be stable across commits?

Not yet. But repack-feature is still usable and stable by definition!

## Does it help if I want to backup the world?

Yep!

This utility can decompress and packet together all chunks so there are no trash.
You can *manually* compress resulting file to get much smaller files.

Also, it's fast.

### Example

```bash
$ du -hs r.10.4.mca
12M r.10.4.mca # BIG! Sad :c

$ anvilregion-repacker -c -i r.10.4.mca -o r.10.4.mca.bin
$ du -hs r.10.4.mca.bin
33M r.10.4.mca.bin # BIGGER! But wait...

$ zstd r.10.4.mca r.10.4.mca.bin
$ du -hs r.10.4.mca{,.bin}.zst
10M     r.10.4.mca.zst # 🦥
2,0M    r.10.4.mca.bin.zst # 🚀🚀🚀
```

## Does it help if I want reduce world size? / Does it help if I want reduce resulting .zip archive with the world?

Yep!

This utility can remove unused sectors and replace unused space with zeroes.
Also, compressing such world will result less size due to zeroes.

Also, it's fast.

### Example

```bash
$ du -hs r.10.4.mca
12M r.10.4.mca # BIG! Sad :c

$ anvilregion-repacker -c -i r.10.4.mca -o r.10.4.mca.bin
$ du -hs r.10.4.mca.bin
33M r.10.4.mca.bin # BIGGER! But wait...

$ anvilregion-repacker -d -i r.10.4.mca.bin -o r.10.4.mca.2
$ du -hs r.10.4.mca{,.2}
12M     r.10.4.mca # 🦥
4,7M    r.10.4.mca.2 # 🚀

# That's the same but without large buffer file
$ anvilregion-repacker -c -i r.10.4.mca | anvilregion-repacker -d r.10.4.mca.bin -o r.10.4.mca.2

$ zstd r.10.4.mca r.10.4.mca.2
$ du -hs r.10.4.mca{,.2}.zst
10M     r.10.4.mca.zst # 🦥
2,6M    r.10.4.mca.2.zst # 🚀🚀🚀
```

# Known issues

+ LZ4 compressed worlds (since 1.20.5) are not supported
+ Corrupted chunks prevents process of an entire region

# How to build

Requirements:
+ Rust

# Related (and probably more recommended)

+ [AnvilPacker](https://github.com/Rafiuth/AnvilPacker) (C#)
+ Probably something else?..
