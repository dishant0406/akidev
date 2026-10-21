// Makes Prowlarr's list of indexers match WANTED. Run by the deploy inside the frontend container
// (it has node), once Prowlarr is healthy:
//
//   docker compose exec -T -e PROWLARR_API_KEY frontend node - < scripts/prowlarr-sync.js
//
// Prowlarr tests a site before it will add it, so one that is down or blocked is reported and left out,
// and the next deploy tries again. Nothing here fails the deploy. Prowlarr keeps the definitions of the
// sites it has up to date by itself.

// The `definitionName` of each site, as in https://github.com/Prowlarr/Indexers/tree/master/definitions/v11.
const WANTED = ["dmhy", "mikan", "limetorrents"];
// Tried from the server and left out: 1337x, the Kickass mirrors, ExtraTorrent, TorrentKitty, UIndex,
// TorrentDownload and TorrentProject are all behind Cloudflare's challenge. A solver (Byparr) got two of
// them added and then timed out on every search from this address, so none is worth a try without an
// address Cloudflare trusts.

const base = process.env.PROWLARR_URL || "http://prowlarr:9696";
const headers = { "X-Api-Key": process.env.PROWLARR_API_KEY || "", "Content-Type": "application/json" };

async function call(method, path, body) {
  const response = await fetch(base + path, {
    method,
    headers,
    body: body && JSON.stringify(body),
    signal: AbortSignal.timeout(90_000),
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  if (!response.ok) throw new Error(Array.isArray(data) ? data[0].errorMessage : `${response.status}`);
  return data;
}

async function main() {
  const [existing, schema, profiles] = await Promise.all([
    call("GET", "/api/v1/indexer"),
    call("GET", "/api/v1/indexer/schema"),
    call("GET", "/api/v1/appprofile"),
  ]);
  const have = new Map(existing.map((indexer) => [indexer.definitionName, indexer]));

  for (const name of WANTED) {
    if (have.has(name)) continue;
    const definition = schema.find((entry) => entry.definitionName === name);
    if (!definition) {
      console.log(`    prowlarr: ${name} has no definition`);
      continue;
    }
    try {
      await call("POST", "/api/v1/indexer", { ...definition, name, enable: true, appProfileId: profiles[0].id });
      console.log(`    prowlarr: added ${name}`);
    } catch (error) {
      console.log(`    prowlarr: could not add ${name}: ${error.message.slice(0, 140)}`);
    }
  }
  for (const [name, indexer] of have) {
    if (WANTED.includes(name)) continue;
    await call("DELETE", `/api/v1/indexer/${indexer.id}`);
    console.log(`    prowlarr: removed ${name}`);
  }
  const now = await call("GET", "/api/v1/indexer");
  console.log(`    prowlarr: ${now.length} of ${WANTED.length} indexers set up`);
  await reportFailures(now);
}

// Prowlarr turns a site off for a while after it fails. Says which ones, and what testing each says
// is wrong, so the deploy log shows why a site is not giving results.
async function reportFailures(indexers) {
  const failing = new Set((await call("GET", "/api/v1/indexerstatus")).map((status) => status.indexerId));
  for (const indexer of indexers.filter((each) => failing.has(each.id))) {
    try {
      await call("POST", "/api/v1/indexer/test", indexer);
      console.log(`    prowlarr: ${indexer.name} was failing, and now passes its test`);
    } catch (error) {
      console.log(`    prowlarr: ${indexer.name} is failing: ${error.message.slice(0, 140)}`);
    }
  }
}

main().catch((error) => console.log(`    prowlarr: could not sync: ${error.message}`));
