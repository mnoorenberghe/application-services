/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    bookmark_sync::{SyncedBookmarkKind, SyncedBookmarkValidity},
    db::PlacesDb,
    error::*,
    storage::RowId,
    types::{SyncGuid, Timestamp},
};
use rusqlite::Row;

use sql_support::{self, ConnExt};
use sync15::ServerTimestamp;
use url::Url;

/// Our prod code never needs to read moz_bookmarks_synced, but our test code
/// does.
/// SyncedBookmarkValue is used in our struct so that we can do "smart"
/// comparisons - if an object created by tests has
/// SyncedBookmarkValue::Unspecified, we don't check the value against the
/// target of the comparison. We use this instead of Option<> so that we
/// can correctly check Option<> fields (ie, so that None isn't ambiguous
/// between "no value specified" and "value is exactly None"
#[derive(Debug)]
pub enum SyncedBookmarkValue<T> {
    Unspecified,
    Specified(T),
}

impl<T> Default for SyncedBookmarkValue<T> {
    fn default() -> Self {
        SyncedBookmarkValue::Unspecified
    }
}

impl<T> PartialEq for SyncedBookmarkValue<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &SyncedBookmarkValue<T>) -> bool {
        match (self, other) {
            (SyncedBookmarkValue::Specified(s), SyncedBookmarkValue::Specified(o)) => s == o,
            _ => true,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct SyncedBookmarkItem {
    pub id: SyncedBookmarkValue<RowId>,
    pub guid: SyncedBookmarkValue<SyncGuid>,
    pub parent_guid: SyncedBookmarkValue<Option<SyncGuid>>,
    pub server_modified: SyncedBookmarkValue<ServerTimestamp>,
    pub needs_merge: SyncedBookmarkValue<bool>,
    pub validity: SyncedBookmarkValue<SyncedBookmarkValidity>,
    pub deleted: SyncedBookmarkValue<bool>,
    pub kind: SyncedBookmarkValue<Option<SyncedBookmarkKind>>,
    pub date_added: SyncedBookmarkValue<Timestamp>,
    pub title: SyncedBookmarkValue<Option<String>>,
    pub place_id: SyncedBookmarkValue<Option<RowId>>,
    pub keyword: SyncedBookmarkValue<Option<String>>,
    pub description: SyncedBookmarkValue<Option<String>>,
    pub load_in_sidebar: SyncedBookmarkValue<Option<bool>>,
    pub smart_bookmark_name: SyncedBookmarkValue<Option<String>>,
    pub feed_url: SyncedBookmarkValue<Option<String>>,
    pub site_url: SyncedBookmarkValue<Option<String>>,
    // Note that url is *not* in the table, but a convenience for tests.
    pub url: SyncedBookmarkValue<Option<Url>>,
    pub tags: SyncedBookmarkValue<Vec<String>>,
}

macro_rules! impl_builder_simple {
    ($builder_name:ident, $T:ty) => {
        pub fn $builder_name(&mut self, val: $T) -> &mut SyncedBookmarkItem {
            self.$builder_name = SyncedBookmarkValue::Specified(val);
            self
        }
    };
}
macro_rules! impl_builder_ref {
    ($builder_name:ident, $T:ty) => {
        pub fn $builder_name<'a>(&'a mut self, val: &$T) -> &'a mut SyncedBookmarkItem {
            self.$builder_name = SyncedBookmarkValue::Specified((*val).clone());
            self
        }
    };
}

macro_rules! impl_builder_opt_ref {
    ($builder_name:ident, $T:ty) => {
        pub fn $builder_name<'a>(&'a mut self, val: Option<&$T>) -> &'a mut SyncedBookmarkItem {
            self.$builder_name = SyncedBookmarkValue::Specified(val.map(|v| v.clone()));
            self
        }
    };
}

macro_rules! impl_builder_opt_string {
    ($builder_name:ident) => {
        pub fn $builder_name<'a>(&'a mut self, val: Option<&str>) -> &'a mut SyncedBookmarkItem {
            self.$builder_name = SyncedBookmarkValue::Specified(val.map(|s| s.to_string()));
            self
        }
    };
}

#[allow(unused)] // not all methods here are currently used.
impl SyncedBookmarkItem {
    // A "builder" pattern, so tests can do `SyncedBookmarkItem::new().title(...).url(...)` etc
    pub fn new() -> SyncedBookmarkItem {
        SyncedBookmarkItem {
            ..Default::default()
        }
    }

    impl_builder_simple!(id, RowId);
    impl_builder_ref!(guid, SyncGuid);
    impl_builder_opt_ref!(parent_guid, SyncGuid);
    impl_builder_simple!(server_modified, ServerTimestamp);
    impl_builder_simple!(needs_merge, bool);
    impl_builder_simple!(validity, SyncedBookmarkValidity);
    impl_builder_simple!(deleted, bool);

    // kind is a bit special because tombstones don't have one.
    pub fn kind(&mut self, kind: SyncedBookmarkKind) -> &mut SyncedBookmarkItem {
        self.kind = SyncedBookmarkValue::Specified(Some(kind));
        self
    }

    impl_builder_simple!(date_added, Timestamp);
    impl_builder_opt_string!(title);

    // no place_id - we use url instead.
    pub fn url<'a>(&'a mut self, url: Option<&str>) -> &'a mut SyncedBookmarkItem {
        let url = url.map(|s| Url::parse(s).expect("should be a valid url"));
        self.url = SyncedBookmarkValue::Specified(url);
        self
    }

    impl_builder_opt_string!(keyword);
    impl_builder_opt_string!(description);
    impl_builder_simple!(load_in_sidebar, Option<bool>);
    impl_builder_opt_string!(smart_bookmark_name);
    impl_builder_opt_string!(feed_url);
    impl_builder_opt_string!(site_url);

    pub fn tags<'a>(&'a mut self, mut tags: Vec<String>) -> &'a mut SyncedBookmarkItem {
        tags.sort();
        self.tags = SyncedBookmarkValue::Specified(tags);
        self
    }

    // Get a record from the DB.
    pub fn get(conn: &PlacesDb, guid: &SyncGuid) -> Result<Option<Self>> {
        Ok(conn.try_query_row(
            "SELECT b.*, p.url, group_concat(t.tag) AS tags
                               FROM moz_bookmarks_synced b
                               LEFT JOIN moz_places p on b.placeId = p.id
                               LEFT JOIN moz_bookmarks_synced_tag_relation r ON r.itemId = b.id
                               LEFT JOIN moz_tags t ON t.id = r.tagId
                               WHERE b.guid = :guid
                               GROUP BY b.id",
            &[(":guid", guid)],
            Self::from_row,
            true,
        )?)
    }

    // Return a new SyncedBookmarkItem from a database row. All values will
    // be SyncedBookmarkValue::Specified.
    fn from_row(row: &Row) -> Result<Self> {
        let mut tags = row
            .get_checked::<_, Option<String>>("tags")?
            .map(|tags| {
                tags.split(',')
                    .map(|t| t.to_owned())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        tags.sort();
        Ok(Self {
            id: SyncedBookmarkValue::Specified(row.get_checked("id")?),
            guid: SyncedBookmarkValue::Specified(row.get_checked("guid")?),
            parent_guid: SyncedBookmarkValue::Specified(row.get_checked("parentGuid")?),
            server_modified: SyncedBookmarkValue::Specified(ServerTimestamp(
                row.get_checked::<_, f64>("serverModified")?,
            )),
            needs_merge: SyncedBookmarkValue::Specified(row.get_checked("needsMerge")?),
            validity: SyncedBookmarkValue::Specified(
                SyncedBookmarkValidity::from_u8(row.get_checked("validity")?)
                    .expect("a valid validity"),
            ),
            deleted: SyncedBookmarkValue::Specified(row.get_checked("isDeleted")?),
            kind: SyncedBookmarkValue::Specified(
                // tombstones have a kind of -1, so get it from the db as i8
                SyncedBookmarkKind::from_u8(row.get_checked::<_, i8>("kind")? as u8).ok(),
            ),
            date_added: SyncedBookmarkValue::Specified(row.get_checked("dateAdded")?),
            title: SyncedBookmarkValue::Specified(row.get_checked("title")?),
            place_id: SyncedBookmarkValue::Specified(row.get_checked("placeId")?),
            keyword: SyncedBookmarkValue::Specified(row.get_checked("keyword")?),
            description: SyncedBookmarkValue::Specified(row.get_checked("description")?),
            load_in_sidebar: SyncedBookmarkValue::Specified(row.get_checked("loadInSidebar")?),
            smart_bookmark_name: SyncedBookmarkValue::Specified(
                row.get_checked("smartBookmarkName")?,
            ),
            feed_url: SyncedBookmarkValue::Specified(row.get_checked("feedUrl")?),
            site_url: SyncedBookmarkValue::Specified(row.get_checked("siteUrl")?),
            url: SyncedBookmarkValue::Specified(
                row.get_checked::<_, Option<String>>("url")?
                    .and_then(|s| Url::parse(&s).ok()),
            ),
            tags: SyncedBookmarkValue::Specified(tags),
        })
    }
}