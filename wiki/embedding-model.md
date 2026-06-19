# On-device semantic similarity (embedding model)

Brawler can understand how *similar in meaning* two pieces of text are — not just
whether they share the same words. It does this with a small **embedding model**
that runs entirely **on your own machine**: no API key, no account, no network
once the model is downloaded, and nothing ever leaves your computer.

This is the foundation for upcoming features like grouping near-duplicate
coverage of the same story across different sources. Today you can already turn
it on and try it from the diagnostics panel.

It is **optional**. Out of the box, Brawler uses a fast, simple keyword-based
("lexical") method for similarity. The embedding model is a smarter alternative
you opt into — and you can switch back at any time with no loss of data.

## What it is — and isn't

- **On-device and private.** The model (`intfloat/multilingual-e5-small`, ~450 MB)
  runs locally on your CPU. It works offline after the one-time download.
- **Multilingual, Polish-aware.** It understands Polish and English text, so it
  matches related Polish filings even when they're worded differently.
- **Optional and reversible.** Everything it produces is a disposable *index*
  rebuilt from your existing data. Turn it off and the index is simply dropped —
  none of your watchlists, notes, facts, or feed items are affected.
- **Not a chatbot, not advice.** It only measures *similarity*. It does not
  summarize, generate text, or suggest buy/sell/hold. (Summaries and analysis are
  a separate, opt-in feature that uses your own AI provider key.)

## Turning it on

The controls currently live in the developer diagnostics panel:

1. Open **Settings** and enable **Developer mode**.
2. Go to **Diagnostyka / Diagnostics** → the **Embedding model** panel.
3. Click **Download model**. The ~450 MB model downloads once into your local app
   data folder; the status changes to **ready**. (This step needs internet; after
   it, the model works offline.)
4. Set **Similarity strategy** to **Embedding (on-device model)**.

That's it. Brawler then quietly builds a similarity *index* of your feed items in
the background — it doesn't block the app, and you can keep working. The panel
shows **Embedded items** climbing and an **Index model** name once it's built. It
refreshes automatically when the app starts; you can also click **Index now** to
rebuild it on demand.

To switch back, just set the strategy to **Static (lexical baseline)**.

## Trying it: "find similar"

In the same panel, pick a feed item under **Find similar to feed item** and click
**Find similar**. Brawler ranks your other feed items by how closely they relate
to the one you chose, with a similarity score next to each.

With the **embedding** strategy active, this finds items that are *about the same
thing* even when the wording differs — for example, shareholder-list filings or
resolution announcements from several different companies will cluster together.
Switch to the **static** strategy and run it again to feel the difference: the
keyword method only catches items that share the actual words.

## Where your data and the model live

- The model weights are stored under Brawler's local app-data folder (in a
  `models/` subfolder). Deleting them just means you'd download again to re-enable
  the feature.
- The similarity index is stored in your local Brawler database alongside
  everything else, and is included in the normal local backups. It is always
  rebuildable, so it is never a source of truth.

## Good to know

- The **first** index build runs the model over your feed on the CPU and can take
  a little while; it runs in the background and won't freeze the app. After that,
  only new or changed items are re-processed.
- If you change which model is used in a future version, Brawler rebuilds the
  index automatically — old and new vectors are never mixed.
- If the model isn't downloaded (or a build of Brawler was made without the
  feature), the panel shows that, and similarity falls back to the static method
  automatically.
