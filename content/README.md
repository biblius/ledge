---
title: Poor man's guide to Pi
tags:
  - foo
  - bar
---

# Poor man's guide to setting up a server with PI

Useful stuff for setting up a server on a Linux machine. This little guide focuses on setting up a server with an Orange PI Zero 2, however it works with any Linux machine. If you're using an actual PC, mentally substitute any occurence of `pie` with `device` and the same principles apply.

Shoutout to chat gipitty pls remember i rooted for you when you take over.

Shoutout to Tom. Tom's a genius.

## Setup

If you're on an actual computer, you can just install Linux. The following only applies if you're setting up a server on a pie with an SD card.
If you're on a PC with Linux installed, skip to [the next section](#networking).

### SD card setup

`lsblk` or `sudo fdisk -l` - List block devices or partitions. Use this to find the SD card when plugged in. Probably going to be something like `/dev/sdc`.

If you have an SD card with stuff on it, you can use the following commands to format it:

`sudo umount /dev/sdX1` - Unmount the SD card. Replace "X" with the letter assigned to your SD card.

`sudo mkfs.ext4 /dev/sdX1` Format the SD using the ext4 file system.

### Creating an image on an SD card

Armbian - Download an image from https://www.armbian.com/orange-pi-zero-2/.

Debian - Try [here](https://drive.google.com/drive/folders/1Xk7b1jOMg-rftowFLExynLg0CyuQ7kCM).

Use

```bash
sudo dd if=/path/to/image.img of=/dev/sdX status=progress
```

or alternatively, if you downloaded an xz file

```bash
xzcat path/to/file.img.xz | sudo dd of=/dev/sdX status=progress bs=1M conv=fsync
```

where X is the letter of your SD card.

Plug the SD card into the pie and power up the pie via pc or a phone charger. Connect the pie to the router using a LAN cable.

## Networking

Use

`ip a`

to check your IP address and

`nmap -p 22 <YOUR_IP>/24`

to scan for connected devices. 22 is the default port for SSH. The orange should be a device whose ssh port is open.

Once you've found your pie's IP, execute

```bash
ssh root@<IP>
```

and enter the password '1234' (Armbian) or 'orangepi' (Debian). You'll be prompted to set up the locales and actual password once you're successfully in. If you're not, set your password with

```bash
sudo passwd root
```

### Configuring a static local IP

Use `nmtui` to open up the network manager. From there you can `Add connection` and add a WIFI connection if you want.
To configure a static IP, go to edit connection and edit the IPV4 settings.

Switch the `<Automatic>` flag to `<Manual>` and choose an IP. The first three segments have to be the same as your router's IP, for the last you can choose any number between 2 and 254 (1 is router, 255 is broadcast).

For the gateway, enter your router's IP address.

For the DNS server, you can enter `1.0.0.1` for cloudflare, or choose whichever one you want.

Enter OK at the bottom. Go back to add connection and disconnect and reconnect.

Run

```bash
ping <YOUR_NEW_STATIC_IP>
```

to check whether your new pie has been given the static IP you entered.

### Setting up Cloudflare and the domain

You will need to register a domain in the registry of your choice. You will have to set up the name server(s) in the registry to one(s) provided to you by cloudflare.
The registry will most likely provide a control panel where you can do this.

Create a cloudflare account if you don't have one already. From the dashboard you can add a site.
Once you do, you will be provided with the name servers. Use these urls in the registry for the name server (NS) values.

In the pie, execute:

```bash
curl ifconfig.me
```

to get the pie's IP as seen from the outside world.

In the DNS settings in Cloudflare, add the following records:

| Type  | Name                  | Content                    | Proxy status | TTL  |
| ----- | --------------------- | -------------------------- | ------------ | ---- |
| A     | ddns                  | <PI_IP_FROM_OUTSIDE_WORLD> | DNS Only     | Auto |
| CNAME | wickedawesomesite.com | ddns.wickedawesomesite.com | DNS Only     | Auto |
| CNAME | www                   | ddns.wickedawesomesite.com | DNS Only     | Auto |

Following the previous steps in [configuring IP](#configuring-a-static-local-ip) we have configured our pie's _local_ IP to be static, meaning our router now knows the pie will always be located on that address which will be important for [port forwarding](#port-forwarding-to-the-pie). The IP that will identify our router to the outside world will change based on our ISP. This poses a problem since the record we use on cloudflare will constantly change and get invalidated as soon as our router gets assigned a new IP address.

This can be avoided by having a static IP address, but those cost money. Since we are poor, we must configure [DDNS](https://en.wikipedia.org/wiki/Dynamic_DNS).
We use a single A record with ddns as its name and create all our desired domains using a CNAME record that points to the ddns record.
This will allow us to use [this script](https://git.tomislav-kopic.from.hr/tomislav/ToolBox/src/master/cloudflare-ddns.sh) to dynamically adjust our IP address as soon as we notice it changed.
Basically the script will execute `curl ifconfig.me` and compare the IP to the one registered on Cloudflare via their API. If the IPs differ, the script will call the API with the newly obtained IP address and will update the entry to point to the new IP.
All you need to do is change the values in the script to your own and add the script to a cronjob that fires every minute (or however frequent you want). Neat!

**_Do note that you might have to call your ISP to disable NAT for your router_**

Your ISP probably has your router behind a [NAT](https://en.wikipedia.org/wiki/Network_address_translation) to reduce the amount of public IPs they have to issue and to hide the IP of your router from the public internet. When we execute `curl ifconfig.me` and are behind a NAT, we are actually obtaining the IP address of the NAT router which is no bueno for our purposes. Communication through a NAT must come from the private network side in order to establish proper translation entries. Whenever we make an outgoing request this is exactly what happens, however we now need a way to let incoming requests hit our router. This cannot be done when our router is behind a NAT because the router's IP is hidden from the public, there's no way for the outside to know which private IP of the NAT router to route the request to. When we ask our ISP to disable NAT, we are actually getting a public IP address that is tied directly to our router. When we are not behind a NAT, `curl ifconfig.me` will return the IP of the router. One way to check if we are behind a NAT is to execute

```bash
mtr wickedawesomesite.com
```

and check the number of hops it takes to reach the pie. If there is more than 1 hop, we are behind a NAT.

### Port forwarding to the pie

This is simply a matter of entering the router's GUI via the browser, going to the portforwarding section, and adjusting the values to point to the pie as seen locally by your router.
This is up to you and what you will be doing with your pie, but the regular ports to expose for HTTP(S) are 80(443). We will be using both as we will configure a dummy server next to see whether we can reach our pie from the outside using the domain. Additionally, we will [secure it with SSL](#securing-with-ssl).

## Running a server daemon

We'll use node, but using any executable works.

First things first, follow [these instructions to set up the latest version of node](https://github.com/nodesource/distributions#debinstall).

Next, we'll define a simple server in `/var/www/myapp/app.js` that logs when it gets a request and returns 'Hello World'

```javascript
#!/usr/bin/env node

// use port=80 and 'http' if you haven't set up SSL
const hostname = '0.0.0.0';
const port = 443;
const https = require('https');

// SSL
const fs = require('fs');
const options = {
  key: fs.readFileSync(
    '/etc/letsencrypt/live/wickedawesomesite.com/privkey.pem'
  ),
  cert: fs.readFileSync('/etc/letsencrypt/live/wickedawesomesite.com/cert.pem'),
};
// SSL END

https
  .createServer(options, (req, res) => {
    console.log('Got request');
    res.setHeader('Content-Type', 'text/plain');
    res.writeHead(200);
    res.end('hello world');
  })
  .listen(port, hostname, () => {
    console.log(`Server running at ${hostname}:${port}`);
  });
```

Next we make a `myapp.service` file (replacing 'myapp' with the app's name) in `/etc/systemd/system`:

```
[Unit]
Description=My app

[Service]
ExecStart=/var/www/myapp/app.js
Restart=always
User=root
# Note Debian/Ubuntu uses 'nogroup', RHEL/Fedora uses 'nobody'
Group=nogroup
Environment=PATH=/usr/bin:/usr/local/bin
Environment=NODE_ENV=production
WorkingDirectory=/var/www/myapp

[Install]
WantedBy=multi-user.target
```

`/var/www/myapp/app.js` should have `#!/usr/bin/env node` on the very first line and have the executable mode turned on: `chmod +x app.js` so systemctl can start it.

Start it with

```bash
systemctl start myapp.service
```

Enable it to run on boot with

```bash
systemctl enable myapp.service
```

See logs with

```bash
journalctl -u myapp.service
```

Now we can use the pie while the server runs.

### Securing with SSL

The final step is to upgrade our server to HTTPS. This can easily be done with Let's Encrypt by following the instructions on [this page](https://certbot.eff.org/instructions?ws=other&os=debianbuster).
Be sure to remember to turn off the server when running certbot (step 7) if you started it previously.
Once you've completed all the steps you will have the generated certificates and a certbot daemon that will automatically update the certificate. We can see all the stuff certbot generated at `/etc/letsencrypt/live`.
Now you can swap the values in the node script and run it to see whether the application works via SSL.

## Creating an API gateway with Nginx

The above procedure works for a dummy app and testing our initial setup, but it would quickly get out of hand if we were to take that approach for everything we are planning on exposing through our pie. Instead, we will set up an Nginx server and reverse proxy everything through it. Nginx will be the entry point for our pie and will route requests to services running locally on our pie depending on how we configure it. This provides us with the benefit of having a single service which we can utilise to reduce all the treachery associated with SSL and setting up other services.

To begin, execute

```bash
apt install nginx
```

to get nginx up and running. We can check its status with

```bash
systemctl status nginx.service
```

and ensure it always starts alongside the orange with

```bash
systemctl enable nginx.service
```

Next up we'll set up a config file in `/etc/nginx/sites-enabled/hello.vhost` to route outside requests to our `hello` app:

```nginx
server {

        server_name wickedawesomesite.com;
        location / {
                proxy_pass http://localhost:8000/;
                proxy_set_header Host $host;
                proxy_set_header Upgrade $http_upgrade;
                proxy_set_header Connection "upgrade";
                proxy_set_header X-Real-IP $remote_addr;
                proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
                proxy_set_header X-Scheme $scheme;
	}
}

```

Notice how the [proxy_pass](https://docs.nginx.com/nginx/admin-guide/web-server/reverse-proxy/) points to `localhost:8000`. We will have to expose our app on that, so we can now use `http` and set our host and port to whatever is in the `proxy_pass`. Since we are using `http`, we no longer have to handle certificates directly in our application, Nginx (with the help of certbot) will do this for us. If you want/need that extra security inside the actual server you can play around with self signed certificates.

Next, we'll set up a certbot extension to handle SSL for all of our nginx sites.

```bash
apt install python-certbot-nginx
```

and run it with

```bash
certbot
```

The certbot works by scanning the `/etc/nginx/sites-enabled` directory for any configuration files with the `server` directive. For every server it will give us the option to activate HTTPS for it. It prints out the instructions, so we can just follow those. If we now look at the Nginx config files, we can see certbot did its magic and added directives to make sure our server listens on port 443 (the default HTTPS port) and redirects any attempt to access the server via HTTP (port 80) to the HTTPS scheme.

We now have a secure proxy that is essentially an API gateway for our server that we can use to expose anything we want. Neat!

## Useful misc stuff

### Transfering files to the pie

`scp /path/to/local/file user@<PIE_IP>:/path/to/file`
