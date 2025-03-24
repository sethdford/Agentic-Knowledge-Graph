use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::query::QueryError;
use aws_sdk_dynamodb::operation::query::{QueryInput, QueryOutput};
use aws_sdk_dynamodb::operation::put_item::{PutItemInput, PutItemOutput, PutItemError};
use aws_sdk_dynamodb::operation::scan::{ScanInput, ScanOutput, builders::ScanFluentBuilder};
use aws_sdk_dynamodb::operation::query::builders::QueryFluentBuilder;
use aws_sdk_dynamodb::operation::put_item::builders::PutItemFluentBuilder;
use async_trait::async_trait;

#[async_trait]
pub trait DynamoDBClient {
    fn query(&self) -> QueryFluentBuilder;
    fn put_item(&self) -> PutItemFluentBuilder;
    fn scan(&self) -> ScanFluentBuilder;
}

#[async_trait]
impl DynamoDBClient for aws_sdk_dynamodb::Client {
    fn query(&self) -> QueryFluentBuilder {
        self.query()
    }

    fn put_item(&self) -> PutItemFluentBuilder {
        self.put_item()
    }

    fn scan(&self) -> ScanFluentBuilder {
        self.scan()
    }
} 