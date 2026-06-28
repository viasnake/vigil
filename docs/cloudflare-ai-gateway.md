# Cloudflare AI Gateway

Vigil implements Cloudflare AI Gateway as its only LLM provider.

The provider uses Cloudflare's REST chat-completions endpoint:

```text
POST https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1/chat/completions
```

It sends:

```text
Authorization: Bearer <CLOUDFLARE_API_TOKEN>
cf-aig-gateway-id: <VIGIL_CLOUDFLARE_GATEWAY_ID>
Content-Type: application/json
```

The request body uses the OpenAI-compatible chat-completions shape with a Cloudflare model name such as `openai/gpt-4.1`.

Vigil asks the model to return only a JSON `ReasoningResult`. The response is parsed from `choices[0].message.content`, schema-validated, and semantically checked before rendering. Invalid model responses fail the investigation instead of being treated as authoritative output.

Reference: [Cloudflare AI Gateway REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/).
