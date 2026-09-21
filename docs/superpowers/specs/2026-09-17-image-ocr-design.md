# Local image OCR search

Approved scope: image clipboard items only. No file/PDF/Word extraction. macOS local Apple Vision engine; Windows explicitly unavailable in this first version (UI disabled with explanation), no Linux support.

Settings: `clipboard.ocr.enabled` defaults false. Enabling applies to newly captured images; an explicit action queues historical images and retries failed ones. `clipboard.ocr.paused` defaults false and pauses processing without hiding already indexed results. Disabling stops scheduling/persistence of running results and excludes OCR from searches. Clearing the OCR index pauses processing and removes derived data only. Original images, clipboard items, favorites, notes, groups and timestamps are never changed by OCR.

Persistence: additive migration with a separate image_ocr table and trigram FTS index. Jobs survive restart. One blocking recognition job at a time; never block clipboard ingestion/search. Bounded input (20 MB, 40 megapixels) and output (100k characters). Failed/missing images are counted and never retried in a tight loop. Results for deleted items or cleared/disabled queues are discarded. Resolve storage paths via ImageStore and active pools via DatabaseState; never carry stale results into a replacement database.

Commands: get_image_ocr_status -> {supported,enabled,paused,total,pending,completed,failed}; queue_image_ocr_history -> same; clear_image_ocr_index -> same. Settings edits use existing update_settings. UI shows switch, pause/resume, history action, clear action and progress counts; both zh-CN/en-US. Use ordinary search box and existing kind/favorite/group filters with accurate pagination/count. OCR text is not sent to the list or clipboard writeback.

Verification: old database upgrade, default-off, CJK short and long OCR search, disable hides derived matches but retains note matches, filters/counts, delete cascades, clear invalidates active jobs, pause/restart behavior, actual Vision fixture recognition, TypeScript/Biome/Rust checks, desktop preferences plus image search smoke test. No personal clipboard material in test artifacts.
