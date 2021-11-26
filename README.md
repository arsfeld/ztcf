# ztcf

ztcf stands for ZeroTier Cloudflare. It's a simple script that lists all members from a 
[Zerotier](https://www.zerotier.com/) network and creates entries in a Cloudflare zone.

To configure, create an `.env` file with the following variables:

```
ZT_NETWORK_ID= # Zerotier network id
ZT_API_TOKEN=  # Zerotier API token
CF_TOKEN=      # Cloudflare token
CF_ZONE_ID=    # Cloudflare zone id
```

Then run with: 

```
cargo run
```

# Limitations / Future work

 * It currently only creates A records, nothing more, nothing less. With time it'll be able to 
   replace, update and delete entries from Cloudflare.
 * Output is non-existing  

# License

Check LICENSE file

# Similar apps

* [ztdns](https://github.com/d4v3y0rk/ztdns): Similar software for Route53, written in JS