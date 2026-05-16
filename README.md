# Flashcard API — Rust + Axum + PostgreSQL

## Stack
- **Axum 0.7** — HTTP framework
- **SQLx 0.7** — async PostgreSQL driver (compile-time query checking)
- **bcrypt** — password hashing
- **jsonwebtoken** — JWT auth (30-day tokens)

---

## Quick start

```bash
# 1. Create database
createdb flashcard

# 2. Run migration
psql flashcard < migrations/001_init.sql

# 3. Configure
cp .env.example .env
# Edit DATABASE_URL and JWT_SECRET

# 4. Run
cargo run
```

Server starts on `http://localhost:3000`

---

## Architecture — per-user overrides

```
flashcards (canonical)
    │
    └── user_card_overrides  (per user: front_text, back_text, level, star, learned, count)
    └── flashcard_tags       (per user: only visible to the user who added)
    └── comments             (per user: is_public=true → visible to everyone)
```

Guests read canonical data. Logged-in users see their overrides merged in via `COALESCE`.

---

## Auth

All protected routes require:
```
Authorization: Bearer <token>
```

---

## API Reference

### Auth
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/auth/register` | — | Register |
| POST | `/auth/login` | — | Login → returns JWT |

**Register / Login body:**
```json
{ "email": "user@example.com", "password": "secret123", "username": "alice" }
```

---

### Flashcards
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/flashcards` | Optional | List cards (guests see canonical) |
| GET | `/flashcards/:id` | Optional | Get single card |
| POST | `/flashcards` | Required | Create canonical card |
| PATCH | `/flashcards/:id/override` | Required | Override front/back/level/star/learned |
| POST | `/flashcards/:id/encounter` | Required | Increment encounter count |

**Query params for GET /flashcards:**
```
lang=zh          # filter by language
level=HSK1       # filter by level
word=你好        # search by word (ILIKE)
tag=grammar      # filter by tag name (user's own tags)
starred=true     # filter starred cards
learned=false    # filter unlearned
page=1
per_page=20
```

**Override body:**
```json
{
  "front_text": "My custom front",
  "back_text": "My custom back",
  "level": "HSK3",
  "star": true,
  "learned": false
}
```
All fields optional — only provided fields are updated.

---

### Collections
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/collections` | Optional | List (guests see public only) |
| GET | `/collections/:id` | Optional | Get one (403 if private + not owner) |
| POST | `/collections` | Required | Create collection |
| PUT | `/collections/:id` | Required | Update (owner only) |
| DELETE | `/collections/:id` | Required | Delete (owner only) |
| GET | `/collections/:id/cards` | Optional | List cards in collection |
| POST | `/collections/:id/cards` | Required | Add card to collection |
| DELETE | `/collections/:id/cards/:card_id` | Required | Remove card |

**Create collection:**
```json
{ "name": "HSK1 Essentials", "description": "...", "is_public": true }
```

---

### Comments
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/flashcards/:id/comments` | Optional | Guests see public only; users see own + public |
| POST | `/flashcards/:id/comments` | Required | Add comment |
| PATCH | `/flashcards/:id/comments/:cid` | Required | Edit own comment |
| DELETE | `/flashcards/:id/comments/:cid` | Required | Delete own comment |

**Comment body:**
```json
{ "body": "This is a hard one!", "is_public": true }
```

Comments with `is_public: false` are visible only to the author.
Comments table has a `word` column (denormalized) for fast word-based search.

---

### Tags
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/tags` | Required | List user's tags + system tags |
| GET | `/flashcards/:id/tags` | Required | Tags user added to this card |
| POST | `/flashcards/:id/tags` | Required | Add tag to card (creates tag if new) |
| DELETE | `/flashcards/:id/tags/:tag_id` | Required | Remove tag from card |

Tags are **user-scoped** — other users cannot see your tags.
`flashcard_tags` table has a `word` column for fast search.

---

### Levels
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/levels` | — | List all levels |
| GET | `/levels?lang=zh` | — | Filter by language |

Returns: `HSK1-6`, `N1-N5`, `A1-C2`, etc.

---

## Visibility rules summary

| Feature | Guest | Auth user |
|---------|-------|-----------|
| Read flashcards | ✓ (canonical) | ✓ (with overrides) |
| front_text override | ✗ | ✓ (private) |
| back_text override | ✗ | ✓ (private) |
| level override | ✗ | ✓ (private) |
| star / learned | ✗ | ✓ (private) |
| encounter_count | ✗ | ✓ (private) |
| Comments (public) | ✓ read | ✓ read + write |
| Comments (private) | ✗ | ✓ own only |
| Tags | ✗ | ✓ own only |
| Collections (public) | ✓ read | ✓ read + write |
| Collections (private) | ✗ | ✓ own only |
