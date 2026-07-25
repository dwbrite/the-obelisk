---
title: "building a cheaper k8s cluster at home"
aliases:
  - "building a cheaper k8s cluster at home"
date: 2023-09-14
tags:
  - etc
  - anm_blog
published: false
category: blog
rating: 3
---

Before we get into it, I'll keep this short and sweet.

I am looking for work.

I'm a software developer specializing in backend development, infrastructure, and devops. I'm also interested in and have explored embedded firmware, rendering, and electrical engineering. Three things I really care about are reproducibility, documentation, and community. Take a look at my [resume](https://dwbrite.com/resume/resume.pdf) if you're hiring - or even if you're not! Heck, send it to your friends.

contact me: dwbrite at gmail dot com

---

At the end of 2022 I was hosting my website on Linode Kubernetes Engine. My monthly spend reached $48/mo for a *slightly* excessive setup - but, in fairness, I was doing a lot of experimentation on Linode.
I had just lost my job at Remediant[^0] and this monthly subscription was enough to make me a *little* uncomfortable while unemployed, so I set a new goal:

Build my own kubernetes cluster that's stronger and "cheaper", so that I could still run my website (and various other servers), should finding a job take too long.

### step 1: hardware

The Orange Pi 5 came out less than two months before I decided to build my own cluster. It's an [8 core little beast](https://browser.geekbench.com/v5/cpu/compare/19357188?baseline=19357599) with 8GB RAM for $90 MSRP, at a time when the Raspberry Pi 4B was going for $200, if you could even find it.

On the surface, OS support seemed pretty good. It was an obvious choice, if a bit overkill - being 3-4x stronger than the RPi. I bought 3 with an estimated ROI period of just under a year - *not including* the benefits of far more capable hardware than I could find from VPS providers.

### step 2: networking

At my apartment in NYC I was behind CGNAT[^1] with NYC Mesh, and IPv6 support was (is) nonexistent. That left me with two options:

1. Giving a third party unencrypted access to all traffic flowing into and out of my cluster (see: Cloudflare Tunnels), or
2. Hosting at my mom's house.

I opted for the latter (thanks mom), which meant progress slowed significantly until I moved back in with her.
When I got there, I upgraded our firewall/router to a mini PC running VyOS. This allowed me to define my [firewall's configuration as code](https://github.com/dwbrite/firenet/blob/master/provisioning/config/config.boot.j2), upload it with Ansible, and not have to manually dig around in some UI for each change. It's similar to Ubiquiti's EdgeOS and Juniper's Junos OS in that way.
I find it incredibly comforting that my network configuration is easy to reproduce or roll back.

### step 3: building the cluster

Before I could think about Kubernetes, I needed an OS to run it on. And before that, I needed to be able to boot from the NVMe drive, which the Orange Pi 5 does not support out of the box. Fortunately the process to boot from NVMe is tolerable enough - just load up an official Orange Pi distro and update the SPI_FLASH bootloader via [orangepi-config](https://jamesachambers.com/orange-pi-5-ssd-boot-guide/).
Once I did that, I installed RebornOS on a USB and wrote a makefile to do some initial config and copy the install to each machine's NVMe drive. I chose RebornOS because it *appeared* to be better supported than other distributions. And honestly, the official Orange Pi distros seemed kind of sketchy[^2].
I opted for k0s as my kubernetes distribution, because it's ridiculously easy to install and it allows me to declaritively define my cluster's topology.
I was also already familiar with Mirantis because of Lens, which certainly helped.
Small hiccup: while k0s does support ARM64, the k0s *controller* can't run on ARM because of some issue with etcd. I didn't look much into it, but...:
Poor man's solution: run the k0s controller on my x86 firewall, and remember to never ever ever (ever) open port 6443.
It took less than an hour to get a proper kubernetes cluster running.

### step 4: system applications

Having worked with kubernetes only in the context of cloud providers, I expected many things to "just work" out of the box.
To test the cluster I attempted to deploy a basic web server with nginx-ingress-controller and cert-manager.
I found out pretty quickly that if I wanted to create a LoadBalancer Service, I'd need a load balancer. On a whim, I installed MetalLB and it worked with minimal configuration. Just as well, I now had a discrete pool of IP addresses I could port forward to.
Then I decided, fuck it, let's learn Istio, and I replaced nginx-ingress-controller. The switch was surprisingly easy, and I'd say it feels slightly cleaner overall.
I also installed ArgoCD so I could manage the applications running on my cluster *without* having to rely strictly on helm. This has the added benefit that I don't have to worry *as much* about my deployments, especially when resources deploy out of order.

### step 5: continuous integration and pain

After I demo'd Istio and friends, I wanted to get my *real* website up. To do that, I needed to build ARM container images for it.
On Github Actions, this took [over an hour](https://github.com/dwbrite/website-rs/actions/runs/5948160261). But I'm impatient, and I had some very capable little ARM machines within walking distance.
*So*, I deployed the github actions runner controller, to control deployment of github actions self-hosted runners. :^)
This was the first time anything on my cluster ~~needed~~ wanted to create persistent volumes, and apparently I did not have a storage provisioner.
*sigh*
So, I installed Ceph+Rook.
My memory of that ordeal is entirely gone, but suffice to say *something* didn't work, and configuration was a bit painful. Then reddit said "Longhorn should be simpler", so I pivoted to that.
I installed it with ArgoCD and helm, and...

> *Bzzt* 🤖
> You don't have iSCSI support! I can't work with this!

Turns out RebornOS for the Orange Pi 5 doesn't have iSCSI kernel modules.
But that's fine I guess. It had been several months since I installed RebornOS, and Joshua Riek's [ubuntu-rockchip](https://github.com/Joshua-Riek/ubuntu-rockchip) distro was really picking up steam. I installed ubuntu-rockchip on one machine and gave Longhorn another go. I set every nodeSelector I could find in Helm to target that machine, but alas-

> *Bzzt* 🤖
> No iSCSI support on "*non-storage machines*", idiot.
> P.S., we don't have a way to set nodeSelector for this specific DaemonSet lol. Try taints and tolerations, I promise that'll work *wink wink*. [^3]

So I set some taints and tolerations, even though I would have *really* preferred to stick with nodeSelectors.

> *Bzzt* 🤖
> Are you fucking stupid?! I can't deploy this pod to "*non-storage machines*", it's tainted!

... 😐

---

So I started from scratch with OpenEBS. Mayastor seemed to be the best storage engine for OpenEBS, *and* it looked easy to configure, *and* it's written in Rust. So it *had* to be a good choice.
One small problem: it doesn't run on ARM.
...[Unless?](https://github.com/openebs/mayastor-extensions/pull/7)
Xinliang Liu - my hero - added ARM support to mayastor-extensions *and* published his images.
With a bit of modification, it [fucking](https://github.com/dwbrite/mayastor-extensions). [worked](https://github.com/dwbrite/openebs-charts)!
Well... Almost.
There was just one more issue. Mayastor relies on nvme_fabric, which is not enabled by default in the linux-rockchip kernel.
So I [enabled it](https://github.com/dwbrite/linux-rockchip), compiled ubuntu-rockchip myself, and finally got persistent volumes working. If you're looking to reproduce this, you can compile ubuntu-rockchip yourself, or use the [image I built](https://github.com/dwbrite/linux-rockchip/releases/tag/ubuntu-opi5%2Fnvme-fabric).
Funny story though, you can actually disable the volume claims on ARC, so none of this was really necessary at the time. *But*, once I start running my plex ser--

> 🤖 On ARM? Haaaahahahahahahaaha

Alright then, once I start running *Jellyfin* on the cluster, *and Outline*, I'll be happy I did all that.

### step 6: blue/green deployments with istio and argocd

Back to happy boring land, I created two ArgoCD Applications which point to blue and green [Kustomize overlays](https://github.com/dwbrite/firenet/tree/master/k8s/apps/website-rs/overlays) for my website's Deployment.
Each overlay points to a specific version/tag[^4] of my website's container image, and labels the resources. All I need to do to switch which one is live is modify the VirtualService and push my changes.
It's downright pleasant.

### now and onwards, and other random thoughts

As painful as *some of this* was, I'm very happy with the outcome, and it made an excellent learning experience.
I know a lot of people dislike kubernetes for being "over-complicated", but once it's configured it makes deploying new applications ridiculously easy. And, I love when my projects are declaritive, automated, and reproducible. This has given me all of that.

---

If you like the way I think, please hire me!
As noted, I've been out of work for 9 months. Finding work in today's market has been extraordinarily tough. Like, *0.5% response rate compared to 9% last year* tough.
I have a number of referrals available upon request, and one testimonial I'm particularly fond of. Closing out after 7 months working for Remediant, a senior developer said to me: "[I] think you've progressed well beyond junior devops at this point which is a testament to your ability and drive."
Thanks Scott :)
If you're looking for a backend engineer with DevOps chops, or junior-(ish?) DevOps engineer, please take a look at my [resume](https://dwbrite.com/resume/resume.pdf) and we can schedule a call.

---

Onwards, for my cluster I have five... six... uh, -- many projects in mind:

- My non-tech friend wants me to host her website; just a simple wordpress site
- Jellyfin, and Some service to upload - or download ;) - media onto Jellyfin's data volume
- Outline Wiki, to store my thoughts, ideas, and collections
- Keycloak/Authelia/Authentik for authentication into cluster-hosted services[^5]
- 3-2-1 Backups.
- Maybe putting an old laptop in the cluster to get x86 support where absolutely needed. Plex, I'm looking at you.
- Hosting a free/public nameserver[^6]
- I'd really like to do *something* with the GPU compute available on the Orange Pi 5.

At some point (maybe once some of these projects are finished) I'd like to fork this project and turn it into an afforable and easily reproducible "infrastructure blueprint", a la [hackerspace.zone](https://hackerspace.zone/Main_Page#).

---

In other news, it seems support for the [Orange Pi 5 in mainline linux](https://lore.kernel.org/lkml/cover.1692632346.git.efectn@6tel.net/T/) is on its way. Currently, all Orange Pi 5 distros are based on Rockchip's custom 5.10 kernel, which as I recall isn't even *really* 5.10 in the first place.

---

The total cost for my cluster was just under $500. That's about $165 including a power supply and 256GB NVMe drive for each machine, plus tax. It's a pretty steep up-front cost, and honestly one machine would have probably sufficed - but then I wouldn't be able to play with DaemonSets or cordon my nodes to update their OS without losing a beat!

Not mentioned earlier, is that I accidentally fried one in a freak wiring incident while attempting to access the serial interface, because I couldn't find my jumper wires. Oops.

The amortized recurring cost of the cluster, given infinite time, is something like $4/mo. But that's only because...

I'm not exposing my home's IP address to the world! I have an nginx proxy on a cheap VPS for about $3.50/mo, and update my home's IP address in that with a [simple script](https://github.com/dwbrite/firenet/blob/master/k8s/system/proxy-networking/files/update_proxy.py).

This also resolves the need for NAT hairpinning, which is [notoriously](https://forum.vyos.io/t/hairpin-nat/178) [difficult](https://www.reddit.com/r/vyos/comments/lohvm8/hairpinnat_reflection_with_dynamic_ip/) in [VyOS](https://forum.vyos.io/t/cannot-get-nat-hairpinning-to-work/7529/5).

---

Fun fact: Chick-fil-A is notorious for running bare-metal kubernetes clusters [at each of their restaurants](https://medium.com/chick-fil-atech/enterprise-restaurant-compute-f5e2fd63d20f) with consumer hardware! I just think this is a really neat idea and a fun piece of kubernetes lore :)

---

[^0]: In December 2022, [Netwrix acquired Remediant](https://www.netwrix.com/netwrix_acquires_remediant_to_provide_customers_with_enhanced_privileged_access_security.html), and dropped the entire SaaS product and team. This included myself.
[^1]: It's actually possible to get a public IPv4 address assigned with NYC Mesh, but it involves talking to people, and I had the intuition I'd be moving in a few months anyway.
[^2]: Well, there's a script that runs on startup which curls baidu to see if the internet is working. Not incriminating, but not inspiring either.
[^3]: IIRC it was the longhorn-engine daemonset I couldn't set the nodeSelector on, and which I [could not](https://github.com/longhorn/charts/blob/3b57266ab050578a858b53b53c355eb3110e12c2/charts/longhorn/Chart.yaml#L18) find a [source for](https://github.com/search?q=repo%3Alonghorn%2Flonghorn-engine%20Chart.yaml&type=code). The [docs](https://longhorn.io/docs/1.5.1/advanced-resources/deploy/node-selector/) did not help. In retrospect, I probably could have found a solution by digging a little deeper - perhaps by looking at the manifests generated in ArgoCD. Alas, I am human with limits on frustration and hope, and OpenEBS was starting to look very compelling.
[^4]: see: [overlays](https://github.com/dwbrite/firenet/blob/master/k8s/apps/website-rs/overlays/blue/kustomization.yaml) and [routing](https://github.com/dwbrite/firenet/blob/master/k8s/apps/website-rs/routing/virtualservice.yaml). This works well enough for blue/green. I'd like to add a few features here, like maybe having blue and green.dwbrite.com so I can see changes before putting them live. Maybe those could only be accessible from LAN (via VPN?). I'd also like to get some canarying/automated deployment going. I have Flagger in mind for this.
[^5]: Outline, Matrix, [maybe Jellyfin](https://github.com/9p4/jellyfin-plugin-sso). I'm not sure what else I'll host.
[^6]: Since discovering that the domain `ns.agency` was available, and then purchasing it, I have become slightly obsessed with this idea of hosting a public nameserver. Public utilities like this are so enchanting to me. If you haven't heart of sdf.org, I recommend checking it out.
