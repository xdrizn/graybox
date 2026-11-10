// Graybox Live Link: marks a GameObject built from a Graybox layer, so a new export updates it in
// place (components you add to it stay) instead of making it again.
using UnityEngine;

namespace Indirect.Graybox
{
    [DisallowMultipleComponent]
    public sealed class GrayboxNode : MonoBehaviour
    {
        [Tooltip("The Graybox layer this object was built from.")]
        public long layerId;

        [Tooltip("The Graybox screen it belongs to.")]
        public long screenId;
    }
}
