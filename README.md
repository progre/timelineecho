## DynamoDB

```bash
aws dynamodb create-table \
  --table-name Config \
  --provisioned-throughput ReadCapacityUnits=1,WriteCapacityUnits=1 \
  --key-schema \
  AttributeName=id,KeyType=HASH \
  --attribute-definitions \
  AttributeName=id,AttributeType=N
aws dynamodb update-table \
  --table-name Config \
  --deletion-protection-enabled
aws dynamodb put-item \
  --table-name Config \
  --item '{
    "id": { "N": "0" },
    "json": { "S": "..." }
  }'
```

## Build

### on Windows

```powershell
cargo lambda build --release --compiler cross
```

### on macOS

```bash
cargo lambda build --release
```

## Deploy

```bash
cargo lambda deploy --profile ???
```
