// Makes Prowlarr's list of indexers match WANTED. Run by the deploy inside the frontend container
// (it has node), once Prowlarr is healthy:
//
//   docker compose exec -T -e PROWLARR_API_KEY frontend node - < scripts/prowlarr-sync.js
//
// Prowlarr tests a site before it will add it, so one that is down or blocked is reported and left out,
// and the next deploy tries again. Nothing here fails the deploy. Prowlarr keeps the definitions of the
// sites it has up to date by itself.
//
// A site marked `solver` sits behind Cloudflare's challenge. It is added with a tag, and Prowlarr sends
// the requests of every tagged site through Byparr, which opens them in a browser and solves it.

// The `definitionName` of each site, as in https://github.com/Prowlarr/Indexers/tree/master/definitions/v11.
const WANTED = [
  { name: "dmhy" },
  { name: "mikan" },
  { name: "limetorrents" },
  { name: "torrentkitty", solver: true },
  { name: "extratorrent-st", solver: true },
];
// Tried from the server through the solver and left out: 1337x and the Kickass mirror time out,
// TorrentDownload is still blocked, and each one tried costs a deploy a minute or more.

const base = process.env.PROWLARR_URL || "http://prowlarr:9696";
const solverAddress = process.env.BYPARR_URL || "http://byparr:8191/";
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

// The id of the tag that sends a site through Byparr, once Prowlarr knows about Byparr. Null when it
// cannot be set up, and then the sites that need it are left out.
async function solverTag() {
  const tags = await call("GET", "/api/v1/tag");
  const tag = tags.find((each) => each.label === "solver") || (await call("POST", "/api/v1/tag", { label: "solver" }));
  const proxies = await call("GET", "/api/v1/indexerProxy");
  if (proxies.some((proxy) => proxy.name === "byparr")) return tag.id;

  const schema = await call("GET", "/api/v1/indexerProxy/schema");
  const flaresolverr = schema.find((entry) => entry.implementation === "FlareSolverr");
  const proxy = {
    ...flaresolverr,
    name: "byparr",
    tags: [tag.id],
    fields: flaresolverr.fields.map((field) => (field.name === "host" ? { ...field, value: solverAddress } : field)),
  };
  // Prowlarr tests it as it saves it, and Byparr downloads its browser when it first starts, so the
  // first answer can take a minute.
  for (let attempt = 1; attempt <= 4; attempt++) {
    try {
      await call("POST", "/api/v1/indexerProxy", proxy);
      console.log("    prowlarr: Byparr set up as the Cloudflare solver");
      return tag.id;
    } catch (error) {
      if (attempt === 4) console.log(`    prowlarr: the Cloudflare solver is not available: ${error.message.slice(0, 140)}`);
      else await new Promise((resolve) => setTimeout(resolve, 20_000));
    }
  }
  return null;
}

async function main() {
  const [existing, schema, profiles] = await Promise.all([
    call("GET", "/api/v1/indexer"),
    call("GET", "/api/v1/indexer/schema"),
    call("GET", "/api/v1/appprofile"),
  ]);
  const have = new Map(existing.map((indexer) => [indexer.definitionName, indexer]));
  const missing = WANTED.filter((site) => !have.has(site.name));
  const tag = missing.some((site) => site.solver) ? await solverTag() : null;

  for (const { name, solver } of missing) {
    const definition = schema.find((entry) => entry.definitionName === name);
    if (!definition) {
      console.log(`    prowlarr: ${name} has no definition`);
      continue;
    }
    if (solver && tag === null) {
      console.log(`    prowlarr: ${name} needs the Cloudflare solver, which is not available`);
      continue;
    }
    try {
      const tags = solver ? [tag] : [];
      await call("POST", "/api/v1/indexer", { ...definition, name, tags, enable: true, appProfileId: profiles[0].id });
      console.log(`    prowlarr: added ${name}`);
    } catch (error) {
      console.log(`    prowlarr: could not add ${name}: ${error.message.slice(0, 140)}`);
    }
  }
  for (const [name, indexer] of have) {
    if (WANTED.some((site) => site.name === name)) continue;
    await call("DELETE", `/api/v1/indexer/${indexer.id}`);
    console.log(`    prowlarr: removed ${name}`);
  }
  const now = await call("GET", "/api/v1/indexer");
  console.log(`    prowlarr: ${now.length} of ${WANTED.length} indexers set up`);
}

main().catch((error) => console.log(`    prowlarr: could not sync: ${error.message}`));
