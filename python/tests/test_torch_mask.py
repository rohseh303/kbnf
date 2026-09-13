"""The torch fast mask path must agree with the reference CPU mask on every available device.

Run with: python -m pytest python/tests -q  (torch optional; MPS/CUDA cases skip when absent)
"""
import pytest

kbnf = pytest.importorskip("kbnf")
torch = pytest.importorskip("torch")


def _engine(vocab_size: int, allowed_fraction: float):
    tokens = {i: kbnf.Token(bytes([65 + (i % 26)])) if i < int(vocab_size * allowed_fraction) else kbnf.Token(bytes([48 + (i % 10)])) for i in range(vocab_size)}
    strings = {i: tokens[i].value.decode() if hasattr(tokens[i], "value") else chr(65 + (i % 26)) for i in range(vocab_size)}
    vocabulary = kbnf.Vocabulary(tokens, {i: chr(65 + (i % 26)) if i < int(vocab_size * allowed_fraction) else chr(48 + (i % 10)) for i in range(vocab_size)})
    engine = kbnf.Engine("start ::= #'[A-Z]+';", vocabulary, kbnf.Config.hardened())
    engine.compute_allowed_token_ids()
    return engine


@pytest.mark.parametrize("device", ["cpu", "mps", "cuda"])
@pytest.mark.parametrize("allowed_fraction", [0.1, 0.9])
def test_fast_mask_matches_reference(device, allowed_fraction):
    if device == "mps" and not torch.backends.mps.is_available():
        pytest.skip("no MPS device")
    if device == "cuda" and not torch.cuda.is_available():
        pytest.skip("no CUDA device")
    engine = _engine(2000, allowed_fraction)
    logits = torch.randn(2000, dtype=torch.float32)
    reference = engine.mask_logits(logits.clone().numpy())
    masked = engine.mask_logits(logits.clone().to(device)).to("cpu")
    assert torch.equal(torch.from_numpy(reference), masked)
    allowed = set(engine.get_allowed_token_ids_from_last_computation())
    assert all((masked[i] == float("-inf")) == (i not in allowed) for i in range(2000))
