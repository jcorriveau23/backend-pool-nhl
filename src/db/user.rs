use mongodb::bson::doc;
use mongodb::Database;
use mongodb::Cursor;
use mongodb::options::FindOneAndUpdateOptions;
use mongodb::options::ReturnDocument;
use futures::stream::TryStreamExt;

use crate::models::user::User;

pub async fn find_user(
    db: &Database,
    _name: String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"name": _name}, None).await.unwrap();

    Ok(user)
}

pub async fn find_users(
    db: &Database,
) -> mongodb::error::Result<Vec<User>> {
    let collection = db.collection::<User>("users");

    let mut cursor = collection.find(None, None).await?;

    let mut users: Vec<User> = vec![];

    while let Some(user) = cursor.try_next().await? {
        users.push(user);
    }

    Ok(users)
}

// pub async fn update_user(
//     db: &Database,
//     _user: UserDB,
// ) -> mongodb::error::Result<Option<UserDB>> {
//     let collection = db.collection::<UserDB>("users");
//     let find_one_and_update_options = FindOneAndUpdateOptions::builder()
//         .return_document(ReturnDocument::After)
//         .build();


//     let filter = doc!{"name": _user.child_members.name};
//     let user = collection.find_one_and_update(filter, _user, find_one_and_update_options).await.unwrap();

//     Ok(user)
// }