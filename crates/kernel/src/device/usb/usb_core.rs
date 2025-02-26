
use super::usb::*;
use super::usbdi::*;


pub struct usb_xfer {

    // struct usb_callout timeout_handle;
	// TAILQ_ENTRY(usb_xfer) wait_entry;	/* used at various places */

	// struct usb_page_cache *buf_fixup;	/* fixup buffer(s) */
	// struct usb_xfer_queue *wait_queue;	/* pointer to queue that we
	// 					 * are waiting on */
	// struct usb_page *dma_page_ptr;
    pub endpoint: *mut usb_endpoint,

    // struct usb_xfer_root *xroot;	/* used by HC driver */
	// void   *qh_start[2];		/* used by HC driver */
	// void   *td_start[2];		/* used by HC driver */
	// void   *td_transfer_first;	/* used by HC driver */
	// void   *td_transfer_last;	/* used by HC driver */
	// void   *td_transfer_cache;	/* used by HC driver */
	// void   *priv_sc;		/* device driver data pointer 1 */
	// void   *priv_fifo;		/* device drive	r data pointer 2 */
	// void   *local_buffer;
	// usb_frlength_t *frlengths;
	// struct usb_page_cache *frbuffers;
	// usb_callback_t *callback;

	// usb_frlength_t max_hc_frame_size;
	// usb_frlength_t max_data_length;
	// usb_frlength_t sumlen;		/* sum of all lengths in bytes */
	// usb_frlength_t actlen;		/* actual length in bytes */
	// usb_timeout_t timeout;		/* milliseconds */

	// usb_frcount_t max_frame_count;	/* initial value of "nframes" after
	// 				 * setup */
	// usb_frcount_t nframes;		/* number of USB frames to transfer */
	// usb_frcount_t aframes;		/* actual number of USB frames
	// 				 * transferred */
	// usb_stream_t stream_id;		/* USB3.0 specific field */

	// uint16_t max_packet_size;
	// uint16_t max_frame_size;
	// uint16_t qh_pos;
	// uint16_t isoc_time_complete;	/* in ms */
	// usb_timeout_t interval;	/* milliseconds */

	// uint8_t	address;		/* physical USB address */
	// uint8_t	endpointno;		/* physical USB endpoint */
	// uint8_t	max_packet_count;
	// uint8_t	usb_state;
	// uint8_t fps_shift;		/* down shift of FPS, 0..3 */

	// usb_error_t error;

	// struct usb_xfer_flags flags;
	// struct usb_xfer_flags_int flags_int;
}