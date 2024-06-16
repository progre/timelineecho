## Build

Local

### on Windows

```powershell
cargo lambda build --release --compiler cross
```

### on macOS

```bash
cargo lambda build --release
```

## Deploy

### Initialize

ChoudShell

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

aws dynamodb create-table \
  --table-name Store \
  --provisioned-throughput ReadCapacityUnits=1,WriteCapacityUnits=1 \
  --key-schema \
  AttributeName=id,KeyType=HASH \
  --attribute-definitions \
  AttributeName=id,AttributeType=N
aws dynamodb update-table \
  --table-name Store \
  --deletion-protection-enabled
aws dynamodb put-item \
  --table-name Store \
  --item '{
    "id": { "N": "0" },
    "json": { "S": "..." }
  }'

aws iam create-user --user-name timelineecho-deploy
aws iam create-access-key --user-name timelineecho-deploy # Save to local
aws iam put-user-policy \
  --user-name timelineecho-deploy \
  --policy-name LambdaDeployPolicy \
  --policy-document '{
  "Version": "2012-10-17",
  "Statement": [
      {
          "Effect": "Allow",
          "Action": [
              "iam:AttachRolePolicy",
              "iam:CreateRole",
              "iam:GetRole",
              "iam:GetRolePolicy",
              "iam:PassRole",
              "iam:UpdateAssumeRolePolicy",
              "lambda:CreateFunction",
              "lambda:GetFunction",
              "lambda:UpdateFunctionCode",
              "lambda:UpdateFunctionConfiguration"
          ],
          "Resource": "*"
      }
  ]
}'

# ----
# Execute the first deployment from local
# ----

aws iam put-user-policy \
  --user-name timelineecho-deploy \
  --policy-name LambdaDeployPolicy \
  --policy-document '{
  "Version": "2012-10-17",
  "Statement": [
      {
          "Effect": "Allow",
          "Action": [
              "lambda:CreateFunction",
              "lambda:GetFunction",
              "lambda:UpdateFunctionCode",
              "lambda:UpdateFunctionConfiguration"
          ],
          "Resource": "*"
      }
  ]
}'

aws lambda update-function-configuration \
  --function-name timelineecho \
  --timeout 100

aws iam attach-role-policy \
  --role-name "$(aws lambda get-function-configuration --function-name timelineecho --query 'Role' --output text | cut -d "/" -f 2)" \
  --policy-arn arn:aws:iam::aws:policy/AmazonDynamoDBFullAccess

# Scheduler

aws iam create-role \
  --role-name timelineecho-scheduler \
  --assume-role-policy-document "{
  \"Version\": \"2012-10-17\",
  \"Statement\": [{
    \"Effect\": \"Allow\",
    \"Principal\": {
      \"Service\": \"scheduler.amazonaws.com\"
    },
    \"Action\": \"sts:AssumeRole\"
  }
]}"
aws iam put-role-policy \
  --role-name "timelineecho-scheduler" \
  --policy-name "timelineecho-scheduler" \
  --policy-document "{
    \"Version\": \"2012-10-17\",
    \"Statement\": [{
        \"Effect\": \"Allow\",
        \"Action\": [
            \"lambda:InvokeFunction\"
        ],
        \"Resource\": [
            \"$(aws lambda get-function-configuration --function-name timelineecho --query 'FunctionArn' --output text)\"
        ]
    }]
}"
aws scheduler create-schedule \
  --name timelineecho \
  --schedule-expression "rate(2 minutes)" \
  --schedule-expression-timezone 'Asia/Tokyo' \
  --flexible-time-window '{ "Mode": "OFF" }' \
  --target "{
  \"RoleArn\": \"$(aws iam get-role --role-name "timelineecho-scheduler" --query 'Role.Arn' --output text)\",
  \"Arn\": \"$(aws lambda get-function-configuration --function-name timelineecho --query 'FunctionArn' --output text)\"
}"

# After execute

aws logs put-retention-policy \
  --log-group-name /aws/lambda/timelineecho \
  --retention-in-days 90
```

Local

```bash
cargo lambda deploy --profile timelineecho-deploy
```
