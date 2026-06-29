# Cloudflare AI Gateway

Vigil implements Cloudflare AI Gateway as its only LLM provider.

By default, the provider uses Cloudflare's REST chat-completions endpoint:

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

Vigil can also use Cloudflare's documented provider-native Gateway endpoint:

```text
POST https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider_path}
```

Set the endpoint to `gateway` with `--endpoint gateway`, `VIGIL_CLOUDFLARE_ENDPOINT=gateway`, or `cloudflare.endpoint = "gateway"` in a config file. For Workers AI models with an `@cf/` model name, Vigil uses:

```text
workers-ai/v1/chat/completions
```

and sends:

```text
cf-aig-authorization: Bearer <CLOUDFLARE_API_TOKEN>
Content-Type: application/json
```

Vigil asks the model to return only a JSON `ReasoningResult`. The response is parsed from `choices[0].message.content`, schema-validated, and semantically checked before rendering. Invalid model responses fail the investigation instead of being treated as authoritative output.

References:

* [Cloudflare AI Gateway REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/)
* [Authenticated Gateway](https://developers.cloudflare.com/ai-gateway/configuration/authentication/)
* [Workers AI through AI Gateway](https://developers.cloudflare.com/ai-gateway/usage/providers/workersai/)
