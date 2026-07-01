# Quick Guide

1. Add the archive mirrors to `.env`:

   ```
   ARCHIVE_URLS=https://annas-archive.gl,https://annas-archive.pk,https://annas-archive.gd
   ```

2. Start shelfd:

   ```bash
   just up
   ```

3. Open your ebook reader (e.g. [Foliate](https://johnfactotum.github.io/foliate/)) and add the OPDS feed:

   ```
   http://localhost:7451/opds
   ```

   In Foliate: `Add Catalog` → paste the URL → done. Browse by subject or search directly from the reader.
