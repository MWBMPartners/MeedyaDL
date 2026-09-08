# Crash reporting — can we host Sentry ourselves, and what are the options?

**Asked:** 2026-09-08 — can Sentry run on DreamHost shared hosting, to be surfaced at
`sentry.mwbm.cloud`? If not, what else, and what would Sentry's own hosting cost?

**Short answer:** self-hosted Sentry on shared hosting is not possible, and not by a small
margin. There are three realistic routes instead. **There is also a prior question worth asking
before spending anything** — see the last section.

---

## 1. Sentry on DreamHost shared hosting — no

Sentry's own requirements for running it yourself:

| Requirement | Stated minimum |
|---|---|
| Docker | Required (Docker 19.03.6+, Docker Compose 2.32.2+) |
| Processor | 4 cores |
| Memory | **16 GB, plus 16 GB swap** (32 GB recommended) |
| Disk | 20 GB free |

Shared hosting offers none of that. There is no Docker, no ability to run background services, no
root access, and nothing close to 16 GB of memory for one customer. Sentry is not one program —
it is a cluster of databases, queues and workers running together on one machine.

This is not a "tight but doable" situation. It is the wrong shape of hosting entirely.

---

## 2. The three realistic routes

### Route A — Sentry's own hosting, free tier

| | |
|---|---|
| Cost | £0 |
| Limits | **1 user**, 5,000 errors per month, 30 days of history |

The event allowance is shared across everything you point at it. The single-user limit is the real
constraint — it is a solo-developer tier, so nobody else can log in to look.

Fine for finding out whether crash reporting is useful before committing. Not something to build a
team habit on.

### Route B — Sentry's own hosting, paid

| Plan | Cost (billed yearly) | What you get |
|---|---|---|
| Team | **$26/month** | Unlimited users, 50,000 errors/month, up to 90 days |
| Business | $80/month | As above plus advanced quota controls and single sign-on |

Team is the sensible paid step. Nothing to run, nothing to keep alive, no upgrades to apply.

### Route C — GlitchTip, hosted by us at `sentry.mwbm.cloud` — the recommended route if we self-host

GlitchTip is an open-source error tracker built as an alternative to Sentry. It is dramatically
lighter:

| | Sentry self-hosted | GlitchTip |
|---|---|---|
| Memory | 16 GB + 16 GB swap | **256 MB minimum, 512 MB typical** |
| Needs | Docker, several databases and queues | PostgreSQL 14+; Redis optional |
| Without Docker | Not supported | Possible, but "not recommended" |

That is a sixty-fold difference in memory. It would run comfortably on a small virtual server —
though **still not on shared hosting**, because it needs a database and a continuously running
process.

### Can GlitchTip run on DreamHost shared hosting? Also no.

Asked directly on 2026-09-08, for a shared plan with no command line for installing software.

**No — and three separate things each rule it out on their own.**

1. **It needs PostgreSQL. Shared hosting gives you MySQL.** This is the hard blocker. GlitchTip
   requires PostgreSQL 14 or newer and does not support MySQL. That is not a setting to change; it
   is a different database engine, and the application is built against it.
2. **It needs a program running all the time.** GlitchTip is a Django web application with
   background workers. Shared hosting serves a page when someone asks for one and then stops.
   There is nowhere for a permanently running process to live.
3. **It is Python, not PHP — so Composer is not the missing piece.** Composer installs PHP
   libraries. GlitchTip needs a Python environment, its own package installer, and a way to run and
   restart the application. GlitchTip's own documentation calls the non-Docker route "not
   recommended" and aimed at people comfortable deploying complex Python applications by hand,
   including the web server, the workers, SSL and upgrades.

**A useful contrast, since both are in flight:** the MWBM Updater is a good fit for shared hosting
— PHP and MySQL, serving a page when asked, nothing running in between. GlitchTip is the opposite
shape. This is not "difficult on shared hosting"; it is the wrong kind of hosting for the job.

**It would run fine on a small virtual server** — 512 MB of memory is genuinely modest, and
DreamHost sell those, as does everyone else.

**Two things to verify before committing to this route:**

1. **That our applications can send to it unchanged.** GlitchTip is designed as a Sentry
   alternative and the intention is that existing Sentry client libraries work by simply pointing
   at a different address. Prove that with one application before planning around it, rather than
   taking it on trust.
2. **What running it actually costs us in attention.** It is our database to back up, our server to
   patch, our upgrades to apply. At $26/month for the paid alternative, the honest comparison is
   not "free versus paid" — it is "our time versus $26".

---

## 3. What each route means in practice

| | Free tier | Team plan | GlitchTip, self-hosted |
|---|---|---|---|
| Money | £0 | ~$312/year | Server cost, likely £4–10/month |
| Our time | None | None | Ongoing — backups, updates, keeping it up |
| Team access | **One person only** | Everyone | Everyone |
| Own address | No | No | Yes — `sentry.mwbm.cloud` |
| Data stays with us | No | No | Yes |
| Can it die quietly? | No | No | **Yes** — and then nobody is told about crashes |

That last row matters more than it looks, given everything found in the release machinery this
week. A crash reporter that has quietly stopped receiving looks exactly like an application with
no crashes. If we host it, it needs the same watching as everything else.

---

## 4. The prior question — do we need this at all?

**MeedyaDL already has crash reporting**, and it does not use Sentry.

When something goes wrong, the app writes a report locally and offers to send it as a GitHub issue,
with the person shown exactly what will be sent and asked to agree first. That already covers the
main purpose: knowing that something broke and having enough detail to fix it.

Sentry has always been a **second, parallel path** in this app, switched off by default, and no
official build has ever had an address configured — so it has never sent anything, to anyone,
ever.

So the real question is not "which hosting?" but **"what does a second crash reporting system give
us that the existing one does not?"** The honest answers are:

- **Crashes nobody reports.** The existing route needs a person to agree to send. Most people
  never do. Sentry-style reporting catches the silent majority.
- **Patterns across many installs.** One report tells you something broke; fifty tell you which
  version, which platform, and how often.
- **The same view across all our applications**, rather than per-repository issues.

Those are genuine benefits. They are also exactly why it carries a privacy obligation, needs a
clear notice, and should stay switched off unless someone chooses it.

---

## 5. Recommendation

1. **Fix the honesty problem first, whatever is decided.** Today a switch and a first-launch popup
   ask people about a feature that cannot function. That is worth correcting on its own, this week,
   and is independent of any hosting choice.
2. **Then try the free tier for one application.** It costs nothing and answers the real question —
   whether the reports are useful enough to justify anything further. The single-user limit is
   tolerable for an experiment.
3. **If it proves useful, choose on time rather than money.** Team hosting at $26/month, or
   GlitchTip at `sentry.mwbm.cloud` if owning the data matters more than the hours.
4. **If we self-host, watch it from day one.** A crash reporter that has silently stopped receiving
   is indistinguishable from good news.

---

## Sources

Checked on 2026-09-08: Sentry's self-hosting requirements page, Sentry's pricing page, and
GlitchTip's installation documentation. **Prices and free-tier limits change** — confirm before
committing money.
