# SSI dive-log vocabularies (global enums)

Captured from the authenticated `mydivelog/add` form + `mydivelog_18_add_details`
snippet (2026-06-25). These are **SSI-global** value→label tables for the
`odin_user_log_*` dropdowns. Account-specific lists (buddies, facility, training
programs/`brevet_rule`, `user_master_id`) are intentionally NOT stored here — fetch
them live per session.

> Verify periodically: SSI can add options. The parser/mapper should treat unknown
> ids gracefully and fall back to free text/comment.

## odin_user_log_var_divetype_id (primary dive type)
| id | label |
|----|-------|
| 23 | Education |
| 24 | Fun Dive |
| 138 | Scientific |
| 139 | Work |

## odin_user_log_var_tanktype_id (and all *_tanktype_id)
| id | label |
|----|-------|
| 19 | Steel |
| 20 | Aluminum |

## odin_user_log_gearconfiguration_id
| id | label |
|----|-------|
| 66 | Single Cylinder (back mount) |
| 67 | Twinset (back mount) |
| 68 | Sidemount Twinset |
| 141 | Sidemount Single |

## odin_user_log_var_water_body_id
| id | label | | id | label |
|----|-------|-|----|-------|
| 13 | Ocean | | 124 | Cave/Cavern |
| 16 | Lake | | 125 | Cavern/Cenote |
| 15 | Quarry | | 52 | Pool/Indoor |
| 18 | Artificial Lake | | 53 | Confined Water |
| 54 | Open Water | | 17 | Indoor |
| 14 | River | | 84 | Dry/Land |
| 123 | Blue Hole | | 140 | Spring |

## odin_user_log_var_entry_id
| id | label |
|----|-------|
| 21 | Shore/Beach |
| 22 | Boat Dive |
| 35 | Other |

## odin_user_log_var_watertype_id
| id | label |
|----|-------|
| 4 | Fresh Water |
| 5 | Salt Water |

## odin_user_log_var_current_id
| id | label |
|----|-------|
| 6 | No Current |
| 7 | Light Current |
| 8 | Strong Current |
| 9 | Ripping Current |

## odin_user_log_var_surface_id
| id | label |
|----|-------|
| 10 | Calm |
| 11 | Moving |
| 12 | Stormy |

## odin_user_log_var_weather_id
| id | label |
|----|-------|
| 1 | Cloudless |
| 2 | Cloudy |
| 3 | Rainy |
| 121 | Snow |

## odin_user_log_var_specialdive_id[] (multi-select; dive tags)
| id | label | | id | label |
|----|-------|-|----|-------|
| 25 | Boat Dive | | 33 | Equipment |
| 26 | Perfect Buoyancy Dive | | 122 | Ice Dive |
| 45 | Cave/Cavern | | 30 | Navigation Dive |
| 156 | Cleanup Dive | | 40 | Night/Limited Vis |
| 27 | Dive Computer | | 43 | Nitrox Dive |
| 127 | Deco Dive | | 32 | Photo and Video Dive |
| 28 | Deep Dive | | 159 | Altitude Diving |
| 153 | Classified Diving | | 38 | Search |
| 148 | Coral ID | | 37 | Shark Dive |
| 157 | Coral Restoration | | 47 | Wreck Dive |
| 152 | Fish ID | | 126 | Open Water Dive |
| 158 | Invasive species management | | 48 | Snorkel Diver |
| 151 | Manta & Ray ID | | 137 | DPV/Scooter |
| 147 | Mine | | 29 | Drift Dive |
| 146 | Overhead Environment | | 31 | Dry Suit Dive |
| 155 | Public Safety Diving | | 149 | Search & Recovery |
| 150 | Diver Stress & Rescue | | 154 | Turtle ID |

## Tech/rebreather field families (same tanktype table)
`odin_user_log_xr_*` (extended range), `odin_user_log_scr_*` (SCR),
`odin_user_log_ccr_*` (CCR: o2/diluent/bailout01-03) each have `_tanktype_id`
(Steel/Aluminum) + unit selects. Map these only for tech/CCR dives.

## Account-specific (fetch live, do NOT hardcode/commit)
- `odin_user_log_user_master_id` — from the add-form hidden field per session.
- `log_linked_facility_id` — the user's affiliated center(s). **Leave BLANK on test
  submissions** so no center is notified.
- `odin_user_log_buddy_ids[]`, `log_linked_brevet_rule_id` — per-account lists.
- `odin_user_log_animal_ids` (wildlife) — loaded per dive-site via AJAX.
