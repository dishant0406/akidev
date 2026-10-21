// Makes Prowlarr's list of indexers match WANTED. Run by the deploy inside the frontend container
// (it has node), once Prowlarr is healthy:
//
//   docker compose exec -T -e PROWLARR_API_KEY frontend node - < scripts/prowlarr-sync.js
//
// Prowlarr tests a site before it will add it, so one that is down or behind Cloudflare is reported
// and left out, and the next deploy tries again. Nothing here fails the deploy. Prowlarr keeps the
// definitions of the sites it has up to date by itself.

// The `definitionName` of each site, as in https://github.com/Prowlarr/Indexers/tree/master/definitions/v11.
const WANTED = [
  "torrentkitty",
  "dmhy",
  "mikan",
  "limetorrents",
  "1337x",
  "kickasstorrents-to",
  "extratorrent-st",
  "torrentdownload",
  "torrentproject2",
  "uindex",
];

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
      console.log(`    prowlarr: could not add ${name}: ${error.message}`);
    }
  }
  for (const [name, indexer] of have) {
    if (WANTED.includes(name)) continue;
    await call("DELETE", `/api/v1/indexer/${indexer.id}`);
    console.log(`    prowlarr: removed ${name}`);
  }
  const now = await call("GET", "/api/v1/indexer");
  console.log(`    prowlarr: ${now.length} of ${WANTED.length} indexers set up`);
}

main().catch((error) => console.log(`    prowlarr: could not sync: ${error.message}`));
